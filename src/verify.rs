//! Schema verification engine.
//!
//! Validates one or more CSV files against a schema, checking that every cell
//! matches its declared column type. Supports tiered reporting (summary/detail),
//! configurable violation limits, and header-mismatch detection.
//!
//! # Complexity
//!
//! Verification is O(n × c) where n is the row count and c is the column count.

use std::{collections::HashMap, path::Path};

use anyhow::{Context, Result, anyhow};
use log::info;

use crate::{
    cli::SchemaVerifyArgs,
    data::parse_typed_value,
    io_utils,
    schema::{ColumnType, Schema},
    table,
};

/// Validates one or more CSV files against a schema, reporting type mismatches
/// and optionally printing an invalid-row detail or summary table.
pub fn execute(args: &SchemaVerifyArgs) -> Result<()> {
    let input_encoding = io_utils::resolve_encoding(args.input_encoding.as_deref())?;
    let schema = Schema::load(&args.schema)
        .with_context(|| format!("Loading schema from {schema:?}", schema = args.schema))?;
    let report_config = args
        .report_invalid
        .as_ref()
        .map(|values| parse_report_invalid_options(values))
        .transpose()?;
    for input in &args.inputs {
        let delimiter = io_utils::resolve_input_delimiter(input, args.delimiter);
        validate_file_against_schema(&schema, input, delimiter, input_encoding, report_config)?;
        info!("✓ {input:?} matches schema");
    }
    Ok(())
}

#[derive(Debug, Clone)]
struct InvalidEntry {
    row_number: usize,
    column_name: String,
    datatype: ColumnType,
    raw_value: String,
    normalized_value: Option<String>,
    reason: String,
}

#[derive(Debug, Clone, Copy)]
struct InvalidReportOptions {
    show_detail: bool,
    show_summary: bool,
    limit: Option<usize>,
}

#[derive(Debug, Clone)]
struct ColumnSummary {
    datatype: ColumnType,
    count: usize,
}

fn parse_report_invalid_options(values: &[String]) -> Result<InvalidReportOptions> {
    let mut show_detail = false;
    let mut show_summary = false;
    let mut limit = None;

    for token in values {
        if token.is_empty() {
            continue;
        }
        let lowered = token.to_ascii_lowercase();
        match lowered.as_str() {
            "detail" => {
                show_detail = true;
            }
            "summary" => {
                show_summary = true;
            }
            _ => {
                if let Ok(parsed) = token.parse::<usize>() {
                    limit = Some(parsed);
                    if !show_detail && !show_summary {
                        show_detail = true;
                    }
                } else {
                    return Err(anyhow!(
                        "Invalid value '{token}' for --report-invalid; expected 'detail', 'summary', or a positive integer limit"
                    ));
                }
            }
        }
    }

    if !show_detail && !show_summary {
        show_summary = true;
    }

    Ok(InvalidReportOptions {
        show_detail,
        show_summary,
        limit,
    })
}

fn validate_file_against_schema(
    schema: &Schema,
    path: &Path,
    delimiter: u8,
    encoding: &'static encoding_rs::Encoding,
    report: Option<InvalidReportOptions>,
) -> Result<()> {
    let mut reader =
        io_utils::open_csv_reader_from_path(path, delimiter, schema.expects_headers())?;
    if schema.expects_headers() {
        let headers = io_utils::reader_headers(&mut reader, encoding)?;
        schema
            .validate_headers(&headers)
            .map_err(|err| anyhow!("Validating headers for {path:?}: {err}"))?;
    }

    let report_cfg = report;
    let detail_enabled = report_cfg.is_some_and(|cfg| cfg.show_detail);
    let summary_enabled = report_cfg.is_some_and(|cfg| cfg.show_summary);
    let report_enabled = detail_enabled || summary_enabled;
    let collection_limit = if detail_enabled {
        report_cfg.and_then(|cfg| cfg.limit).unwrap_or(usize::MAX)
    } else {
        0
    };

    let mut invalid_entries = Vec::new();
    let mut column_summary: HashMap<String, ColumnSummary> = HashMap::new();
    let mut total_errors = 0usize;

    for (row_idx, record) in reader.byte_records().enumerate() {
        let record = record.with_context(|| format!("Reading row {} in {path:?}", row_idx + 2))?;
        let decoded = io_utils::decode_record(&record, encoding)?;
        let mut transformed = decoded.clone();
        if schema.has_transformations() {
            schema
                .apply_transformations_to_row(&mut transformed)
                .with_context(|| {
                    format!(
                        "Applying datatype mappings to row {} in {path:?}",
                        row_idx + 2
                    )
                })?;
        }
        schema.apply_replacements_to_row(&mut transformed);
        for (col_idx, column) in schema.columns.iter().enumerate() {
            let raw_value = decoded.get(col_idx).map(|s| s.as_str()).unwrap_or("");
            let normalized_value = transformed.get(col_idx).map(|s| s.as_str()).unwrap_or("");
            if let Err(err) = validate_value(normalized_value, &column.datatype) {
                if !report_enabled {
                    let message = if normalized_value == raw_value {
                        format!(
                            "Row {} column '{}': value {:?}\nReason: {}",
                            row_idx + 2,
                            column.output_name(),
                            raw_value,
                            err
                        )
                    } else {
                        format!(
                            "Row {} column '{}': value {:?} (normalized {:?})\nReason: {}",
                            row_idx + 2,
                            column.output_name(),
                            raw_value,
                            normalized_value,
                            err
                        )
                    };
                    return Err(anyhow!(message));
                }

                let normalized_owned = normalized_value.to_string();
                let normalized_changed = normalized_owned != raw_value;

                total_errors += 1;
                if detail_enabled && invalid_entries.len() < collection_limit {
                    let normalized_value = if normalized_changed {
                        Some(normalized_owned.clone())
                    } else {
                        None
                    };
                    invalid_entries.push(InvalidEntry {
                        row_number: row_idx + 2,
                        column_name: column.output_name().to_string(),
                        datatype: column.datatype.clone(),
                        raw_value: raw_value.to_string(),
                        normalized_value,
                        reason: err.to_string(),
                    });
                }

                column_summary
                    .entry(column.output_name().to_string())
                    .and_modify(|summary| summary.count += 1)
                    .or_insert_with(|| ColumnSummary {
                        datatype: column.datatype.clone(),
                        count: 1,
                    });
            }
        }
    }

    if report_enabled && total_errors > 0 {
        if let Some(cfg) = report_cfg {
            print_invalid_report(path, &invalid_entries, &column_summary, total_errors, cfg);
        }
        return Err(anyhow!(format!(
            "Found {total_errors} invalid value(s) in {path:?}"
        )));
    }

    Ok(())
}

fn validate_value(value: &str, column_type: &ColumnType) -> Result<()> {
    if value.is_empty() {
        return Ok(());
    }
    parse_typed_value(value, column_type).map(|_| ())
}

fn print_invalid_report(
    path: &Path,
    entries: &[InvalidEntry],
    column_summary: &HashMap<String, ColumnSummary>,
    total_errors: usize,
    options: InvalidReportOptions,
) {
    println!();

    if options.show_detail {
        println!("Invalid rows in {}:", path.display());

        let displayed = entries.len();

        if entries.is_empty() {
            println!(
                "No sample rows captured; use '--report-invalid:detail <LIMIT>' with a higher LIMIT to display examples."
            );
        } else {
            let headers = vec![
                "row".to_string(),
                "column".to_string(),
                "raw".to_string(),
                "value".to_string(),
                "datatype".to_string(),
                "reason".to_string(),
            ];

            let rows = entries
                .iter()
                .map(|entry| {
                    let highlight_target =
                        entry.normalized_value.as_ref().unwrap_or(&entry.raw_value);
                    let highlighted = highlight_red(highlight_target);

                    vec![
                        entry.row_number.to_string(),
                        entry.column_name.clone(),
                        entry.raw_value.clone(),
                        highlighted,
                        entry.datatype.to_string(),
                        entry.reason.clone(),
                    ]
                })
                .collect::<Vec<_>>();

            table::print_table(&headers, &rows);
        }

        if total_errors > displayed {
            println!();
            let limit_text = match options.limit {
                None => "no limit".to_string(),
                Some(value) => value.to_string(),
            };
            println!(
                "Displayed {displayed} of {total_errors} invalid row(s) (limit: {limit_text})."
            );
        }

        if options.show_summary && !column_summary.is_empty() {
            println!();
        }
    }

    if options.show_summary && !column_summary.is_empty() {
        println!("Columns with schema violations:");
        let mut summary_rows = column_summary
            .iter()
            .map(|(name, summary)| {
                vec![
                    name.clone(),
                    summary.datatype.to_string(),
                    summary.count.to_string(),
                ]
            })
            .collect::<Vec<_>>();
        summary_rows.sort_by(|a, b| a[0].cmp(&b[0]));

        let headers = vec![
            "column".to_string(),
            "datatype".to_string(),
            "errors".to_string(),
        ];
        table::print_table(&headers, &summary_rows);
        println!();
    } else if options.show_detail {
        println!();
    }
}

fn highlight_red(value: &str) -> String {
    const RED: &str = "\u{1b}[31m";
    const RESET: &str = "\u{1b}[0m";
    format!("{RED}{value}{RESET}")
}
