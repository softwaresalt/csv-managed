use std::collections::HashSet;
use std::env;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use anyhow::{Context, Result, anyhow};
use chrono::Utc;
use log::info;
use serde::Serialize;
use sha2::{Digest, Sha256};
use similar::TextDiff;

use crate::{
    cli::{
        NaPlaceholderBehavior, SchemaArgs, SchemaColumnsArgs, SchemaInferArgs, SchemaMode,
        SchemaProbeArgs, SchemaVerifyArgs, SnapshotFormat,
    },
    columns, io_utils, printable_delimiter,
    schema::{self, ColumnMeta, ColumnType, InferenceStats, Schema, ValueReplacement},
    table, verify, yaml_provider,
};

pub fn execute(args: &SchemaArgs) -> Result<()> {
    match &args.mode {
        Some(SchemaMode::Probe(probe_args)) => execute_probe(probe_args),
        Some(SchemaMode::Infer(infer_args)) => execute_infer(infer_args),
        Some(SchemaMode::Verify(verify_args)) => execute_verify(verify_args),
        Some(SchemaMode::Columns(columns_args)) => execute_columns(columns_args),
        None => execute_manual(args),
    }
}

fn execute_manual(args: &SchemaArgs) -> Result<()> {
    if args.columns.is_empty() {
        return Err(anyhow!(
            "At least one --column definition is required unless using the 'probe' or 'infer' subcommands"
        ));
    }

    let mut columns = parse_columns(&args.columns)
        .with_context(|| "Parsing --column definitions for schema creation".to_string())?;
    apply_replacements(&mut columns, &args.replacements)
        .with_context(|| "Parsing --replace definitions for schema creation".to_string())?;

    let output = required_output_path(
        args.output.as_deref(),
        "An --output path is required for schema creation",
    )?;
    let schema = Schema {
        columns,
        schema_version: None,
        has_headers: true,
    };
    schema
        .save(output)
        .with_context(|| format!("Writing schema to {output:?}"))?;

    info!(
        "Defined schema with {} column(s) written to {:?}",
        schema.columns.len(),
        output
    );

    Ok(())
}

fn resolve_placeholder_policy(args: &SchemaProbeArgs) -> schema::PlaceholderPolicy {
    match args.na_behavior {
        NaPlaceholderBehavior::Empty => schema::PlaceholderPolicy::TreatAsEmpty,
        NaPlaceholderBehavior::Fill => {
            let fill = args
                .na_fill
                .as_deref()
                .map(|value| value.trim())
                .filter(|value| !value.is_empty())
                .unwrap_or("");
            schema::PlaceholderPolicy::FillWith(fill.to_string())
        }
    }
}

fn execute_probe(args: &SchemaProbeArgs) -> Result<()> {
    let input = &args.input;
    let delimiter = io_utils::resolve_input_delimiter(input, args.delimiter);
    let encoding = io_utils::resolve_encoding(args.input_encoding.as_deref())?;
    let placeholder_policy = resolve_placeholder_policy(args);
    info!(
        "Inferring schema from '{}' using delimiter '{}'",
        input.display(),
        printable_delimiter(delimiter)
    );

    let (mut schema, stats) = schema::infer_schema_with_stats(
        input,
        args.sample_rows,
        delimiter,
        encoding,
        &placeholder_policy,
        args.assume_header,
    )
    .with_context(|| format!("Inferring schema from {input:?}"))?;

    let overrides = apply_overrides(&mut schema, &args.overrides)?;

    let suggested_renames = if args.mapping {
        Some(apply_default_name_mappings(&mut schema))
    } else {
        None
    };

    let report = render_probe_report(
        &schema,
        &stats,
        &overrides,
        args.sample_rows,
        &placeholder_policy,
        suggested_renames.as_ref(),
    );
    print!("{report}");
    let snapshot_request = SnapshotRequest {
        path: args.snapshot.as_deref(),
        format: args.snapshot_format,
        notes: args.snapshot_notes.as_deref(),
        schema: &schema,
        report: &report,
        input,
    };
    handle_snapshot(&snapshot_request)?;

    Ok(())
}

fn execute_infer(args: &SchemaInferArgs) -> Result<()> {
    let probe = &args.probe;
    let input_path = &probe.input;
    let delimiter = io_utils::resolve_input_delimiter(input_path, probe.delimiter);
    let encoding = io_utils::resolve_encoding(probe.input_encoding.as_deref())?;
    let placeholder_policy = resolve_placeholder_policy(probe);
    info!(
        "Inferring schema from '{}' using delimiter '{}'",
        input_path.display(),
        printable_delimiter(delimiter)
    );

    let (mut schema, stats) = schema::infer_schema_with_stats(
        input_path,
        probe.sample_rows,
        delimiter,
        encoding,
        &placeholder_policy,
        probe.assume_header,
    )
    .with_context(|| format!("Inferring schema from {input_path:?}"))?;

    let overrides = apply_overrides(&mut schema, &probe.overrides)?;

    let suggested_renames = if probe.mapping {
        Some(apply_default_name_mappings(&mut schema))
    } else {
        None
    };

    let diff_request = if let Some(path) = args.diff.as_deref() {
        Some((
            path.to_path_buf(),
            fs::read_to_string(path)
                .with_context(|| format!("Reading existing schema for diff from {path:?}"))?,
        ))
    } else {
        None
    };

    let mut report: Option<String> = None;

    if let Some(snapshot_path) = probe.snapshot.as_deref() {
        let report_ref = report.get_or_insert_with(|| {
            render_probe_report(
                &schema,
                &stats,
                &overrides,
                probe.sample_rows,
                &placeholder_policy,
                suggested_renames.as_ref(),
            )
        });
        if !args.preview {
            print!("{report_ref}");
        }
        let snapshot_request = SnapshotRequest {
            path: Some(snapshot_path),
            format: probe.snapshot_format,
            notes: probe.snapshot_notes.as_deref(),
            schema: &schema,
            report: report_ref,
            input: input_path,
        };
        handle_snapshot(&snapshot_request)?;
    }

    let preview_requested = args.preview;
    let should_write = !preview_requested && (args.output.is_some() || args.replace_template);
    let diff_requested = diff_request.is_some();

    if preview_requested && let Some(path) = args.output.as_deref() {
        info!("Preview requested; suppressing write to {:?}", path);
    }

    let apply_replacements = preview_requested || should_write || diff_requested;
    let replacements_added = if apply_replacements {
        schema::apply_placeholder_replacements(&mut schema, &stats, &placeholder_policy)
    } else {
        0
    };
    if replacements_added > 0 {
        info!(
            "Added {} NA placeholder replacement(s) to schema",
            replacements_added
        );
    }

    let mut yaml_output = if preview_requested || diff_requested {
        Some(
            schema
                .to_yaml_string(args.replace_template)
                .with_context(|| "Serializing inferred schema to YAML".to_string())?,
        )
    } else {
        None
    };

    if preview_requested {
        println!();
        println!("Schema YAML Preview (not written):");
        let yaml = yaml_output
            .as_deref()
            .expect("Preview requires serialized YAML output");
        print!("{yaml}");
        if !yaml.ends_with('\n') {
            println!();
        }
        info!(
            "Previewed schema for {} column(s) (no file written)",
            schema.columns.len()
        );
    } else if should_write {
        let output = required_output_path(
            args.output.as_deref(),
            "An --output path is required when writing an inferred schema",
        )?;
        if args.replace_template {
            schema
                .save_with_replace_template(output)
                .with_context(|| format!("Writing schema to {output:?}"))?;
        } else {
            schema
                .save(output)
                .with_context(|| format!("Writing schema to {output:?}"))?;
        }
        info!(
            "Inferred schema for {} column(s) written to {:?}",
            schema.columns.len(),
            output
        );
    } else {
        info!(
            "Inferred schema for {} column(s) (no output file written)",
            schema.columns.len()
        );
    }

    if let Some((diff_path, existing_content)) = &diff_request {
        if yaml_output.is_none() {
            yaml_output = Some(
                schema
                    .to_yaml_string(args.replace_template)
                    .with_context(|| "Serializing inferred schema to YAML".to_string())?,
            );
        }
        let new_yaml = yaml_output
            .as_ref()
            .expect("Diff requires serialized YAML output");
        println!();
        if existing_content == new_yaml {
            println!(
                "Schema Diff vs {}: no changes detected.",
                diff_path.display()
            );
        } else {
            println!("Schema Diff vs {}:", diff_path.display());
            let diff = TextDiff::from_lines(existing_content, new_yaml);
            let diff_text = diff
                .unified_diff()
                .context_radius(3)
                .header(&format!("{}", diff_path.display()), "(inferred)")
                .to_string();
            if diff_text.is_empty() {
                println!("(differences detected, but diff output was empty)");
            } else {
                print!("{diff_text}");
                if !diff_text.ends_with('\n') {
                    println!();
                }
            }
        }
    }

    if probe.mapping {
        if preview_requested {
            println!();
        }
        emit_mappings(&schema);
    }

    if let Some(base) = args.evolution_base.as_deref() {
        let evolution_output = resolve_evolution_output(args)?;
        emit_schema_evolution(base, &schema, &evolution_output)?;
    }

    Ok(())
}

fn execute_verify(args: &SchemaVerifyArgs) -> Result<()> {
    verify::execute(args)
}

fn execute_columns(args: &SchemaColumnsArgs) -> Result<()> {
    columns::execute(args)
}

fn required_output_path<'a>(output: Option<&'a Path>, message: &str) -> Result<&'a Path> {
    output.ok_or_else(|| anyhow!(message.to_string()))
}

fn parse_columns(specs: &[String]) -> Result<Vec<ColumnMeta>> {
    let mut columns = Vec::new();
    let mut seen = HashSet::new();
    let mut output_names = HashSet::new();

    for raw in specs {
        for token in raw.split(',') {
            let token = token.trim();
            if token.is_empty() {
                continue;
            }
            let (name_part, type_part) = token.split_once(':').ok_or_else(|| {
                anyhow!("Column definition '{token}' must use the form name:type")
            })?;

            let name = name_part.trim();
            if name.is_empty() {
                return Err(anyhow!(
                    "Column name cannot be empty in definition '{token}'"
                ));
            }
            if !seen.insert(name.to_string()) {
                return Err(anyhow!("Duplicate column name '{name}' provided"));
            }

            let (type_raw, rename_raw) = if let Some((ty, rename)) = type_part.split_once("->") {
                (ty, Some(rename))
            } else {
                (type_part, None)
            };

            let column_type = ColumnType::from_str(type_raw.trim())
                .map_err(|err| anyhow!("Column '{name}' has invalid type '{type_part}': {err}"))?;

            let rename = rename_raw
                .map(|value| value.trim())
                .filter(|value| !value.is_empty())
                .map(|value| value.to_string());

            if let Some(ref alias) = rename {
                if alias != name && seen.contains(alias) {
                    return Err(anyhow!(
                        "Output name '{alias}' conflicts with an existing column name"
                    ));
                }
                if !output_names.insert(alias.clone()) {
                    return Err(anyhow!("Duplicate output column name '{alias}' provided"));
                }
            }

            if rename.is_none() {
                output_names.insert(name.to_string());
            }

            columns.push(ColumnMeta {
                name: name.to_string(),
                datatype: column_type,
                rename,
                value_replacements: Vec::new(),
                datatype_mappings: Vec::new(),
            });
        }
    }

    if columns.is_empty() {
        return Err(anyhow!("At least one --column definition is required"));
    }

    Ok(columns)
}

fn apply_replacements(columns: &mut [ColumnMeta], specs: &[String]) -> Result<()> {
    if specs.is_empty() {
        return Ok(());
    }
    let mut lookup = HashSet::new();
    for column in columns.iter() {
        lookup.insert(column.name.clone());
    }

    for raw in specs {
        let spec = raw.trim();
        if spec.is_empty() {
            continue;
        }
        let (column_name, mapping) = spec.split_once('=').ok_or_else(|| {
            anyhow!("Replacement '{spec}' must use the form column=value->new_value")
        })?;
        let column_name = column_name.trim();
        if column_name.is_empty() {
            return Err(anyhow!("Replacement '{spec}' is missing a column name"));
        }
        if !lookup.contains(column_name) {
            return Err(anyhow!(
                "Replacement references unknown column '{column_name}'"
            ));
        }
        let (from_raw, to_raw) = mapping.split_once("->").ok_or_else(|| {
            anyhow!(
                "Replacement '{spec}' must include '->' to separate original and replacement values"
            )
        })?;
        let from = from_raw.trim().to_string();
        let to = to_raw.trim().to_string();
        let column = columns
            .iter_mut()
            .find(|c| c.name == column_name)
            .expect("column should exist");
        if let Some(existing) = column
            .value_replacements
            .iter()
            .position(|r| r.from == from)
        {
            column.value_replacements.remove(existing);
        }
        column
            .value_replacements
            .push(ValueReplacement { from, to });
    }

    Ok(())
}

fn apply_overrides(schema: &mut Schema, overrides: &[String]) -> Result<HashSet<String>> {
    if overrides.is_empty() {
        return Ok(HashSet::new());
    }

    let mut seen = HashSet::new();
    let mut applied = HashSet::new();
    for raw in overrides {
        let spec = raw.trim();
        if spec.is_empty() {
            continue;
        }
        let (name_part, type_part) = spec
            .split_once(':')
            .ok_or_else(|| anyhow!("Override '{spec}' must use the form name:type"))?;
        let name = name_part.trim();
        if name.is_empty() {
            return Err(anyhow!("Override '{spec}' is missing a column name"));
        }
        if !seen.insert(name.to_string()) {
            return Err(anyhow!("Duplicate override provided for column '{name}'"));
        }

        let override_type = ColumnType::from_str(type_part.trim()).with_context(|| {
            format!("Override for column '{name}' has invalid type '{type_part}'")
        })?;

        let column = schema
            .columns
            .iter_mut()
            .find(|col| col.name == name)
            .ok_or_else(|| anyhow!("Override references unknown column '{name}'"))?;
        column.datatype = override_type;
        applied.insert(name.to_string());
    }

    Ok(applied)
}

fn apply_default_name_mappings(schema: &mut Schema) -> HashSet<String> {
    let mut suggested = HashSet::new();
    for column in &mut schema.columns {
        if column.rename.is_none() {
            let suggestion = to_lower_snake_case(&column.name);
            column.rename = Some(suggestion);
            suggested.insert(column.name.clone());
        }
    }
    suggested
}

fn render_probe_report(
    schema: &Schema,
    stats: &InferenceStats,
    overrides: &HashSet<String>,
    requested_sample_rows: usize,
    placeholder_policy: &schema::PlaceholderPolicy,
    suggested_renames: Option<&HashSet<String>>,
) -> String {
    if schema.columns.is_empty() {
        return "No columns inferred.\n".to_string();
    }
    let rows_read = stats.rows_read();
    let headers = vec![
        "#".to_string(),
        "name".to_string(),
        "type".to_string(),
        "rename".to_string(),
        "override".to_string(),
        "sample".to_string(),
        "format".to_string(),
        "observations".to_string(),
    ];
    let mut rows = Vec::with_capacity(schema.columns.len());
    for (idx, column) in schema.columns.iter().enumerate() {
        let rename_display = column
            .rename
            .as_deref()
            .filter(|value| !value.is_empty())
            .map(|value| {
                if suggested_renames.is_some_and(|set| set.contains(&column.name)) {
                    format!("{value} (suggested)")
                } else {
                    value.to_string()
                }
            })
            .unwrap_or_else(|| "—".to_string());
        let mut status_flags = Vec::new();
        if overrides.contains(&column.name) {
            status_flags.push("type");
        }
        if column.rename.is_some() {
            status_flags.push("mapping");
        }
        let status_display = if status_flags.is_empty() {
            "—".to_string()
        } else {
            status_flags.join("+")
        };
        let sample_display = stats
            .sample_value(idx)
            .map(truncate_sample)
            .unwrap_or_else(|| "—".to_string());
        let format_display = schema::format_hint_for(&column.datatype, stats.sample_value(idx))
            .unwrap_or_else(|| "—".to_string());
        let observation_display = column_observation_summary(stats, idx, rows_read);
        rows.push(vec![
            (idx + 1).to_string(),
            column.name.clone(),
            column.datatype.to_string(),
            rename_display,
            status_display,
            sample_display,
            format_display,
            observation_display,
        ]);
    }
    let mut output = table::render_table(&headers, &rows);

    if requested_sample_rows == 0 {
        output.push_str(&format!("\nSampled {rows_read} row(s) (full scan).\n"));
    } else if rows_read >= requested_sample_rows {
        output.push_str(&format!(
            "\nSampled {rows_read} row(s) (requested limit {requested_sample_rows}).\n"
        ));
    } else {
        output.push_str(&format!(
            "\nSampled {rows_read} row(s) out of requested {requested_sample_rows}.\n"
        ));
    }
    if stats.decode_errors() > 0 {
        output.push_str(&format!(
            "Skipped {} value(s) due to decoding errors.\n",
            stats.decode_errors()
        ));
    } else {
        output.push_str("No decoding errors encountered.\n");
    }

    let signature = compute_schema_signature(schema);
    output.push_str(&format!("Header+Type Hash: {signature}\n"));

    if let Some(section) = render_placeholder_section(schema, stats, placeholder_policy) {
        output.push_str(&section);
    }

    output
}

fn truncate_sample(value: &str) -> String {
    const LIMIT: usize = 32;
    let mut result = String::new();
    for (idx, ch) in value.chars().enumerate() {
        if idx >= LIMIT {
            result.push('…');
            break;
        }
        result.push(ch);
    }
    result
}

fn summarize_histogram_value(value: &str) -> String {
    let mut sanitized = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '\n' | '\r' | '\t' => sanitized.push(' '),
            _ => sanitized.push(ch),
        }
    }
    truncate_sample(&sanitized)
}

fn column_observation_summary(
    stats: &InferenceStats,
    column_index: usize,
    rows_read: usize,
) -> String {
    let mut fragments = Vec::new();
    if let Some(summary) = stats.summary(column_index) {
        fragments.push(format!("non_empty={}", summary.non_empty));
        let empty = rows_read.saturating_sub(summary.non_empty);
        if rows_read > 0 && empty > 0 {
            fragments.push(format!("empty={empty}"));
        }
        if !summary.tracked_values.is_empty() {
            let histogram = summary
                .tracked_values
                .iter()
                .take(3)
                .map(|(value, count)| {
                    let display = summarize_histogram_value(value);
                    format!("{display} ({count})")
                })
                .collect::<Vec<_>>()
                .join(", ");
            fragments.push(format!("samples=[{histogram}]"));
            if summary.tracked_values.len() > 3 {
                fragments.push("samples+=...".to_string());
            }
        }
        if summary.other_values > 0 {
            fragments.push(format!("others={}", summary.other_values));
        }
    }

    if let Some(placeholders) = stats.placeholder_summary(column_index) {
        let entries = placeholders.entries();
        if !entries.is_empty() {
            let tokens = entries
                .iter()
                .take(3)
                .map(|(token, count)| {
                    let display = truncate_sample(token);
                    format!("{display} ({count})")
                })
                .collect::<Vec<_>>()
                .join(", ");
            fragments.push(format!("placeholders=[{tokens}]"));
            if entries.len() > 3 {
                fragments.push("placeholders+=...".to_string());
            }
        }
    }

    if fragments.is_empty() {
        "—".to_string()
    } else {
        fragments.join("; ")
    }
}

fn render_placeholder_section(
    schema: &Schema,
    stats: &InferenceStats,
    placeholder_policy: &schema::PlaceholderPolicy,
) -> Option<String> {
    let mut blocks = Vec::new();
    for (idx, column) in schema.columns.iter().enumerate() {
        let Some(summary) = stats.placeholder_summary(idx) else {
            continue;
        };
        let entries = summary.entries();
        if entries.is_empty() {
            continue;
        }
        let mut block = String::new();
        let type_note = if column.datatype != ColumnType::String {
            " (non-string)"
        } else {
            ""
        };
        block.push_str(&format!(
            "  • {} ({}{})\n",
            column.name, column.datatype, type_note
        ));
        let tokens = entries
            .iter()
            .map(|(token, count)| format!("{token} ({count})"))
            .collect::<Vec<_>>()
            .join(", ");
        block.push_str(&format!("    tokens: {tokens}\n"));
        block.push_str("    replacements:\n");
        let target_display = match placeholder_policy {
            schema::PlaceholderPolicy::TreatAsEmpty => "\"\"".to_string(),
            schema::PlaceholderPolicy::FillWith(value) => format!("\"{value}\""),
        };
        for (token, _) in entries {
            block.push_str(&format!(
                "      - from \"{token}\" -> to {target_display}\n"
            ));
        }
        blocks.push(block);
    }

    if blocks.is_empty() {
        return None;
    }

    let mut section = match placeholder_policy {
        schema::PlaceholderPolicy::TreatAsEmpty => {
            "\nPlaceholder Suggestions (replace with empty string):\n".to_string()
        }
        schema::PlaceholderPolicy::FillWith(value) => {
            format!("\nPlaceholder Suggestions (replace with '{value}'):\n")
        }
    };
    for block in blocks {
        section.push_str(&block);
    }
    Some(section)
}

fn compute_schema_signature(schema: &Schema) -> String {
    let mut hasher = Sha256::new();
    for column in &schema.columns {
        hasher.update(column.name.as_bytes());
        hasher.update(b":");
        hasher.update(column.datatype.signature_token().as_bytes());
        hasher.update(b";");
    }
    format!("{:x}", hasher.finalize())
}

fn emit_mappings(schema: &Schema) {
    if schema.columns.is_empty() {
        println!("No columns found to emit mappings.");
        return;
    }
    let mut rows = Vec::with_capacity(schema.columns.len());
    for (idx, column) in schema.columns.iter().enumerate() {
        let mapping = format!("{}:{}->", column.name, column.datatype.cli_token());
        let suggested = column
            .rename
            .as_ref()
            .filter(|value| !value.is_empty())
            .cloned()
            .unwrap_or_else(|| to_lower_snake_case(&column.name));
        rows.push(vec![
            (idx + 1).to_string(),
            column.name.clone(),
            column.datatype.to_string(),
            mapping,
            suggested,
        ]);
    }
    let headers = vec![
        "#".to_string(),
        "name".to_string(),
        "type".to_string(),
        "mapping".to_string(),
        "suggested".to_string(),
    ];
    table::print_table(&headers, &rows);
}

struct SnapshotRequest<'a> {
    path: Option<&'a Path>,
    format: SnapshotFormat,
    notes: Option<&'a str>,
    schema: &'a Schema,
    report: &'a str,
    input: &'a Path,
}

#[derive(Serialize)]
struct Snapshot {
    metadata: SnapshotMetadata,
    header_hash: String,
    columns: Vec<SnapshotColumn>,
    sample_summary: String,
}

#[derive(Serialize)]
struct SnapshotMetadata {
    timestamp: String,
    version: String,
    source_file: String,
    command: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    notes: Option<String>,
}

#[derive(Serialize)]
struct SnapshotColumn {
    name: String,
    position: usize,
    inferred_type: String,
}

fn handle_snapshot(request: &SnapshotRequest<'_>) -> Result<()> {
    let Some(path) = request.path else {
        return Ok(());
    };

    match request.format {
        SnapshotFormat::Text => write_text_snapshot(path, request.report),
        SnapshotFormat::Json => write_json_snapshot(path, request),
    }
}

fn write_text_snapshot(path: &Path, report: &str) -> Result<()> {
    if path.exists() {
        let expected =
            fs::read_to_string(path).with_context(|| format!("Reading snapshot from {path:?}"))?;
        if expected != report {
            return Err(anyhow!(
                "Probe output does not match snapshot at {path:?}. Inspect differences and update the snapshot if the change is intentional."
            ));
        }
        return Ok(());
    }

    ensure_snapshot_dir(path)?;
    fs::write(path, report).with_context(|| format!("Writing snapshot to {path:?}"))?;
    eprintln!("Snapshot captured at {path:?}");
    Ok(())
}

fn write_json_snapshot(path: &Path, request: &SnapshotRequest<'_>) -> Result<()> {
    let payload = build_snapshot_payload(request);
    let serialized = serde_json::to_string_pretty(&payload)?;
    if path.exists() {
        let expected =
            fs::read_to_string(path).with_context(|| format!("Reading snapshot from {path:?}"))?;
        if expected != serialized {
            return Err(anyhow!(
                "Snapshot JSON does not match {path:?}. Inspect differences and update the snapshot if intentional."
            ));
        }
        return Ok(());
    }
    ensure_snapshot_dir(path)?;
    fs::write(path, serialized).with_context(|| format!("Writing snapshot JSON to {path:?}"))?;
    eprintln!("Snapshot captured at {path:?}");
    Ok(())
}

fn build_snapshot_payload(request: &SnapshotRequest<'_>) -> Snapshot {
    let metadata = SnapshotMetadata {
        timestamp: Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
        version: env!("CARGO_PKG_VERSION").to_string(),
        source_file: request.input.display().to_string(),
        command: env::args().collect::<Vec<_>>().join(" "),
        notes: request.notes.map(|value| value.to_string()),
    };
    let columns = request
        .schema
        .columns
        .iter()
        .enumerate()
        .map(|(idx, column)| SnapshotColumn {
            name: column.name.clone(),
            position: idx + 1,
            inferred_type: column.datatype.to_string(),
        })
        .collect();
    Snapshot {
        metadata,
        header_hash: compute_schema_signature(request.schema),
        columns,
        sample_summary: request.report.to_string(),
    }
}

fn ensure_snapshot_dir(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent)
            .with_context(|| format!("Creating snapshot directory {parent:?}"))?;
    }
    Ok(())
}

fn resolve_evolution_output(args: &SchemaInferArgs) -> Result<PathBuf> {
    if let Some(path) = args.evolution_output.as_ref() {
        return Ok(path.clone());
    }
    let Some(output) = args.output.as_deref() else {
        return Err(anyhow!(
            "--evolution-output is required when --evolution-base is provided without --output"
        ));
    };
    let mut stem = output
        .file_stem()
        .map(|stem| stem.to_os_string())
        .unwrap_or_else(|| OsString::from("schema"));
    stem.push(".evo.yml");
    Ok(output.with_file_name(stem))
}

fn emit_schema_evolution(base_path: &Path, current: &Schema, output: &Path) -> Result<()> {
    let previous = Schema::load(base_path)
        .with_context(|| format!("Loading evolution base schema from {base_path:?}"))?;
    let evolution = schema::evolution::SchemaEvolution::diff(&previous, current);
    if let Some(parent) = output.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent)
            .with_context(|| format!("Creating evolution directory {parent:?}"))?;
    }
    yaml_provider::save_to_path(output, &evolution)
        .with_context(|| format!("Writing schema evolution report to {output:?}"))?;
    info!(
        "Schema evolution report with {} change(s) written to {:?}",
        evolution.changes.len(),
        output
    );
    Ok(())
}

fn to_lower_snake_case(value: &str) -> String {
    let mut result = String::new();
    let mut chars = value.chars().peekable();
    let mut last_was_separator = true;
    let mut last_was_upper = false;
    while let Some(ch) = chars.next() {
        if ch.is_ascii_alphanumeric() {
            if ch.is_ascii_uppercase() {
                let next_is_lowercase = chars
                    .peek()
                    .map(|c| c.is_ascii_lowercase())
                    .unwrap_or(false);
                if !result.is_empty()
                    && (!last_was_separator && (!last_was_upper || next_is_lowercase))
                    && !result.ends_with('_')
                {
                    result.push('_');
                }
                result.push(ch.to_ascii_lowercase());
                last_was_separator = false;
                last_was_upper = true;
            } else {
                if !result.is_empty() && last_was_separator && !result.ends_with('_') {
                    result.push('_');
                }
                result.push(ch.to_ascii_lowercase());
                last_was_separator = false;
                last_was_upper = false;
            }
        } else {
            if !result.ends_with('_') && !result.is_empty() {
                result.push('_');
            }
            last_was_separator = true;
            last_was_upper = false;
        }
    }
    while result.ends_with('_') {
        result.pop();
    }
    if result.is_empty() {
        value.to_ascii_lowercase()
    } else {
        result
    }
}

#[doc(hidden)]
pub mod test_support {
    use super::*;

    pub fn parse_columns(specs: &[&str]) -> Result<Vec<ColumnMeta>> {
        let owned = specs.iter().map(|value| value.to_string()).collect::<Vec<_>>();
        super::parse_columns(&owned)
    }

    pub fn apply_replacements(columns: &mut [ColumnMeta], specs: &[&str]) -> Result<()> {
        let owned = specs.iter().map(|value| value.to_string()).collect::<Vec<_>>();
        super::apply_replacements(columns, &owned)
    }

    pub fn apply_overrides(schema: &mut Schema, specs: &[&str]) -> Result<Vec<String>> {
        let owned = specs.iter().map(|value| value.to_string()).collect::<Vec<_>>();
        let applied = super::apply_overrides(schema, &owned)?;
        Ok(applied.into_iter().collect())
    }

    pub fn apply_default_name_mappings(schema: &mut Schema) -> Vec<(String, String)> {
        let suggested = super::apply_default_name_mappings(schema);
        let mut collected = Vec::with_capacity(suggested.len());
        for original in suggested {
            if let Some(rename) = schema
                .columns
                .iter()
                .find(|column| column.name == original)
                .and_then(|column| column.rename.clone())
            {
                collected.push((original, rename));
            }
        }
        collected.sort_by(|a, b| a.0.cmp(&b.0));
        collected
    }

    pub fn to_lower_snake_case(value: &str) -> String {
        super::to_lower_snake_case(value)
    }
}
