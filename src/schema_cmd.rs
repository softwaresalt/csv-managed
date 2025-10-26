use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::str::FromStr;

use anyhow::{Context, Result, anyhow};
use log::info;
use sha2::{Digest, Sha256};

use crate::{
    cli::{
        SchemaArgs, SchemaColumnsArgs, SchemaInferArgs, SchemaMode, SchemaProbeArgs,
        SchemaVerifyArgs,
    },
    columns, io_utils, printable_delimiter,
    schema::{self, ColumnMeta, ColumnType, InferenceStats, Schema, ValueReplacement},
    table, verify,
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

fn execute_probe(args: &SchemaProbeArgs) -> Result<()> {
    let input = &args.input;
    let delimiter = io_utils::resolve_input_delimiter(input, args.delimiter);
    let encoding = io_utils::resolve_encoding(args.input_encoding.as_deref())?;
    info!(
        "Inferring schema from '{}' using delimiter '{}'",
        input.display(),
        printable_delimiter(delimiter)
    );

    let (mut schema, stats) =
        schema::infer_schema_with_stats(input, args.sample_rows, delimiter, encoding)
            .with_context(|| format!("Inferring schema from {input:?}"))?;

    let overrides = apply_overrides(&mut schema, &args.overrides)?;

    if args.mapping {
        apply_default_name_mappings(&mut schema);
    }

    let report = render_probe_report(&schema, &stats, &overrides, args.sample_rows);
    print!("{report}");
    handle_snapshot(&report, args.snapshot.as_deref())?;

    if args.mapping {
        emit_mappings(&schema);
    }

    Ok(())
}

fn execute_infer(args: &SchemaInferArgs) -> Result<()> {
    let probe = &args.probe;
    let input_path = &probe.input;
    let delimiter = io_utils::resolve_input_delimiter(input_path, probe.delimiter);
    let encoding = io_utils::resolve_encoding(probe.input_encoding.as_deref())?;
    info!(
        "Inferring schema from '{}' using delimiter '{}'",
        input_path.display(),
        printable_delimiter(delimiter)
    );

    let (mut schema, stats) =
        schema::infer_schema_with_stats(input_path, probe.sample_rows, delimiter, encoding)
            .with_context(|| format!("Inferring schema from {input_path:?}"))?;

    let overrides = apply_overrides(&mut schema, &probe.overrides)?;

    if probe.mapping {
        apply_default_name_mappings(&mut schema);
    }

    if let Some(snapshot_path) = probe.snapshot.as_deref() {
        let report = render_probe_report(&schema, &stats, &overrides, probe.sample_rows);
        print!("{report}");
        handle_snapshot(&report, Some(snapshot_path))?;
    }

    let should_write = args.output.is_some() || args.replace_template;
    if should_write {
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

    if probe.mapping {
        emit_mappings(&schema);
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

fn apply_default_name_mappings(schema: &mut Schema) {
    for column in &mut schema.columns {
        if column.rename.is_none() {
            column.rename = Some(to_lower_snake_case(&column.name));
        }
    }
}

fn render_probe_report(
    schema: &Schema,
    stats: &InferenceStats,
    overrides: &HashSet<String>,
    requested_sample_rows: usize,
) -> String {
    if schema.columns.is_empty() {
        return "No columns inferred.\n".to_string();
    }
    let headers = vec![
        "#".to_string(),
        "name".to_string(),
        "type".to_string(),
        "rename".to_string(),
        "override".to_string(),
        "sample".to_string(),
        "format".to_string(),
    ];
    let mut rows = Vec::with_capacity(schema.columns.len());
    for (idx, column) in schema.columns.iter().enumerate() {
        let rename_display = column
            .rename
            .as_deref()
            .filter(|value| !value.is_empty())
            .map(|value| value.to_string())
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
        rows.push(vec![
            (idx + 1).to_string(),
            column.name.clone(),
            column.datatype.to_string(),
            rename_display,
            status_display,
            sample_display,
            format_display,
        ]);
    }
    let mut output = table::render_table(&headers, &rows);

    let rows_read = stats.rows_read();
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

    output.push_str("\nDatatype Map:\n");
    for column in &schema.columns {
        output.push_str(&format!("  • {} -> {}\n", column.name, column.datatype));
    }

    output.push_str("\nColumn Summaries:\n");
    for (idx, column) in schema.columns.iter().enumerate() {
        if let Some(summary) = stats.summary(idx) {
            let mut fragments = Vec::new();
            fragments.push(format!("non_empty={}", summary.non_empty));
            let empty = stats.rows_read().saturating_sub(summary.non_empty);
            if stats.rows_read() > 0 && empty > 0 {
                fragments.push(format!("empty={empty}"));
            }
            if !summary.tracked_values.is_empty() {
                let histogram = summary
                    .tracked_values
                    .iter()
                    .map(|(value, count)| {
                        let display = summarize_histogram_value(value);
                        format!("{display} ({count})")
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                fragments.push(format!("samples=[{histogram}]"));
            }
            if summary.other_values > 0 {
                fragments.push(format!("others={}", summary.other_values));
            }
            if fragments.is_empty() {
                fragments.push("no observed values".to_string());
            }
            output.push_str(&format!("  • {}: {}\n", column.name, fragments.join("; ")));
        }
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
        rows.push(vec![
            (idx + 1).to_string(),
            column.name.clone(),
            column.datatype.to_string(),
            mapping,
        ]);
    }
    let headers = vec![
        "#".to_string(),
        "name".to_string(),
        "type".to_string(),
        "mapping".to_string(),
    ];
    table::print_table(&headers, &rows);
}

fn handle_snapshot(report: &str, snapshot_path: Option<&Path>) -> Result<()> {
    let Some(path) = snapshot_path else {
        return Ok(());
    };

    if path.exists() {
        let expected =
            fs::read_to_string(path).with_context(|| format!("Reading snapshot from {path:?}"))?;
        if expected != report {
            return Err(anyhow!(
                "Probe output does not match snapshot at {path:?}. Inspect differences and update the snapshot if the change is intentional."
            ));
        }
    } else {
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            fs::create_dir_all(parent)
                .with_context(|| format!("Creating snapshot directory {parent:?}"))?;
        }
        fs::write(path, report).with_context(|| format!("Writing snapshot to {path:?}"))?;
        eprintln!("Snapshot captured at {path:?}");
    }

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_columns_accepts_comma_and_repeats() {
        let specs = vec![
            "id:integer,name:string".to_string(),
            "amount:float".to_string(),
        ];
        let columns = parse_columns(&specs).expect("parsed");
        assert_eq!(columns.len(), 3);
        assert_eq!(columns[0].name, "id");
        assert_eq!(columns[1].name, "name");
        assert_eq!(columns[2].name, "amount");
        assert_eq!(columns[0].datatype, ColumnType::Integer);
        assert_eq!(columns[1].datatype, ColumnType::String);
        assert_eq!(columns[2].datatype, ColumnType::Float);
    }

    #[test]
    fn duplicate_columns_are_rejected() {
        let specs = vec!["id:integer,id:string".to_string()];
        let err = parse_columns(&specs).unwrap_err();
        assert!(err.to_string().contains("Duplicate column name"));
    }

    #[test]
    fn missing_type_is_rejected() {
        let specs = vec!["id".to_string()];
        let err = parse_columns(&specs).unwrap_err();
        assert!(err.to_string().contains("must use the form"));
    }

    #[test]
    fn parse_columns_supports_output_rename() {
        let specs = vec!["id:integer->Identifier,name:string".to_string()];
        let columns = parse_columns(&specs).expect("parsed");
        assert_eq!(columns.len(), 2);
        assert_eq!(columns[0].rename.as_deref(), Some("Identifier"));
        assert!(columns[1].rename.is_none());
    }

    #[test]
    fn duplicate_output_names_are_rejected() {
        let specs = vec![
            "id:integer->Identifier".to_string(),
            "code:string->Identifier".to_string(),
        ];
        let err = parse_columns(&specs).unwrap_err();
        assert!(err.to_string().contains("Duplicate output column name"));
    }

    #[test]
    fn replacements_apply_to_columns() {
        let specs = vec!["status:string".to_string()];
        let mut columns = parse_columns(&specs).expect("parsed");
        let replacements = vec!["status=pending->shipped".to_string()];
        apply_replacements(&mut columns, &replacements).expect("applied");
        assert_eq!(columns[0].value_replacements.len(), 1);
        assert_eq!(columns[0].value_replacements[0].from, "pending");
        assert_eq!(columns[0].value_replacements[0].to, "shipped");
    }

    #[test]
    fn replacements_validate_column_names() {
        let specs = vec!["status:string".to_string()];
        let mut columns = parse_columns(&specs).expect("parsed");
        let replacements = vec!["missing=pending->shipped".to_string()];
        let err = apply_replacements(&mut columns, &replacements).unwrap_err();
        assert!(err.to_string().contains("unknown column"));
    }

    #[test]
    fn to_lower_snake_case_converts_names() {
        assert_eq!(to_lower_snake_case("OrderDate"), "order_date");
        assert_eq!(to_lower_snake_case("customer-name"), "customer_name");
        assert_eq!(to_lower_snake_case("customer  name"), "customer_name");
        assert_eq!(to_lower_snake_case("APIKey"), "api_key");
        assert_eq!(to_lower_snake_case("HTTPStatus"), "http_status");
    }

    #[test]
    fn apply_overrides_updates_types() {
        let mut schema = Schema {
            columns: vec![ColumnMeta {
                name: "amount".to_string(),
                datatype: ColumnType::Float,
                rename: None,
                value_replacements: Vec::new(),
                datatype_mappings: Vec::new(),
            }],
            schema_version: None,
        };
        let overrides = vec!["amount:integer".to_string(), "".to_string()];
        let applied = apply_overrides(&mut schema, &overrides).unwrap();
        assert_eq!(schema.columns[0].datatype, ColumnType::Integer);
        assert!(applied.contains("amount"));
    }
}
