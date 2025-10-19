use std::collections::HashMap;

use anyhow::{Context, Result, anyhow};
use encoding_rs::Encoding;
use log::info;

use crate::{
    cli::StatsArgs,
    data::{Value, parse_typed_value},
    io_utils,
    schema::{self, ColumnType, Schema},
    table,
};

pub fn execute(args: &StatsArgs) -> Result<()> {
    if args.schema.is_none() && io_utils::is_dash(&args.input) {
        return Err(anyhow!(
            "Reading from stdin requires --schema (or --meta) for typed statistics"
        ));
    }

    let delimiter = io_utils::resolve_input_delimiter(&args.input, args.delimiter);
    let encoding = io_utils::resolve_encoding(args.input_encoding.as_deref())?;

    let schema = load_or_infer_schema(args, delimiter, encoding)?;

    let columns = resolve_columns(&schema, &args.columns)?;
    if columns.is_empty() {
        return Err(anyhow!(
            "No numeric columns available. Provide a schema file or explicit column list."
        ));
    }

    let mut reader = io_utils::open_csv_reader_from_path(&args.input, delimiter, true)?;
    let headers = io_utils::reader_headers(&mut reader, encoding)?;
    schema
        .validate_headers(&headers)
        .with_context(|| format!("Validating headers for {:?}", args.input))?;

    let mut stats = StatsAccumulator::new(&columns, &schema);

    for (row_idx, record) in reader.byte_records().enumerate() {
        if args.limit > 0 && row_idx >= args.limit {
            break;
        }
        let record = record.with_context(|| format!("Reading row {}", row_idx + 2))?;
        let mut decoded = io_utils::decode_record(&record, encoding)?;
        schema.apply_replacements_to_row(&mut decoded);
        stats
            .ingest(&schema, &decoded)
            .with_context(|| format!("Processing row {}", row_idx + 2))?;
    }

    let rows = stats.render_rows();
    let headers = vec![
        "column".to_string(),
        "count".to_string(),
        "min".to_string(),
        "max".to_string(),
        "mean".to_string(),
        "median".to_string(),
        "std_dev".to_string(),
    ];
    table::print_table(&headers, &rows);
    info!("Computed summary statistics for {} column(s)", rows.len());
    Ok(())
}

fn load_or_infer_schema(
    args: &StatsArgs,
    delimiter: u8,
    encoding: &'static Encoding,
) -> Result<Schema> {
    if let Some(path) = &args.schema {
        Schema::load(path).with_context(|| format!("Loading schema from {path:?}"))
    } else {
        schema::infer_schema(&args.input, 0, delimiter, encoding)
            .with_context(|| format!("Inferring schema from {input:?}", input = args.input))
    }
}

fn resolve_columns(schema: &Schema, specified: &[String]) -> Result<Vec<usize>> {
    if specified.is_empty() {
        Ok(schema
            .columns
            .iter()
            .enumerate()
            .filter(|(_, col)| matches!(col.datatype, ColumnType::Integer | ColumnType::Float))
            .map(|(idx, _)| idx)
            .collect())
    } else {
        specified
            .iter()
            .map(|name| {
                let idx = schema
                    .column_index(name)
                    .ok_or_else(|| anyhow!("Column '{name}' not found in schema"))?;
                let column = &schema.columns[idx];
                if !matches!(column.datatype, ColumnType::Integer | ColumnType::Float) {
                    return Err(anyhow!(
                        "Column '{}' is type {:?} and cannot be profiled as numeric",
                        column.output_name(),
                        column.datatype
                    ));
                }
                Ok(idx)
            })
            .collect()
    }
}

struct StatsAccumulator {
    columns: Vec<usize>,
    data: HashMap<usize, ColumnStats>,
}

impl StatsAccumulator {
    fn new(columns: &[usize], schema: &Schema) -> Self {
        let mut data = HashMap::new();
        for idx in columns {
            let stats = ColumnStats::with_name(schema.columns[*idx].output_name().to_string());
            data.insert(*idx, stats);
        }
        Self {
            columns: columns.to_vec(),
            data,
        }
    }

    fn ingest(&mut self, schema: &Schema, record: &[String]) -> Result<()> {
        for column_index in &self.columns {
            let column = &schema.columns[*column_index];
            let value = record.get(*column_index).map(|s| s.as_str()).unwrap_or("");
            let normalized = column.normalize_value(value);
            if normalized.is_empty() {
                continue;
            }
            if let Some(parsed) = parse_typed_value(normalized.as_ref(), &column.datatype)
                .with_context(|| format!("Column '{}'", column.output_name()))?
            {
                let numeric = match parsed {
                    Value::Integer(i) => i as f64,
                    Value::Float(f) => f,
                    other => {
                        return Err(anyhow!(
                            "Column '{}' expected numeric type but encountered {:?}",
                            column.output_name(),
                            other
                        ));
                    }
                };
                if let Some(stats) = self.data.get_mut(column_index) {
                    stats.add(numeric);
                }
            }
        }
        Ok(())
    }

    fn render_rows(&self) -> Vec<Vec<String>> {
        let mut rows = Vec::new();
        for column_index in &self.columns {
            if let Some(stats) = self.data.get(column_index) {
                rows.push(vec![
                    stats.name.clone(),
                    stats.count.to_string(),
                    stats
                        .min
                        .map(format_number)
                        .unwrap_or_else(|| "".to_string()),
                    stats
                        .max
                        .map(format_number)
                        .unwrap_or_else(|| "".to_string()),
                    stats
                        .mean()
                        .map(format_number)
                        .unwrap_or_else(|| "".to_string()),
                    stats
                        .median()
                        .map(format_number)
                        .unwrap_or_else(|| "".to_string()),
                    stats
                        .std_dev()
                        .map(format_number)
                        .unwrap_or_else(|| "".to_string()),
                ]);
            }
        }
        rows
    }
}

#[derive(Default)]
struct ColumnStats {
    name: String,
    values: Vec<f64>,
    sum: f64,
    sum_squares: f64,
    count: usize,
    min: Option<f64>,
    max: Option<f64>,
}

impl ColumnStats {
    fn with_name(name: String) -> Self {
        Self {
            name,
            ..Self::default()
        }
    }

    fn add(&mut self, value: f64) {
        self.count += 1;
        self.sum += value;
        self.sum_squares += value * value;
        self.min = Some(match self.min {
            Some(current) => current.min(value),
            None => value,
        });
        self.max = Some(match self.max {
            Some(current) => current.max(value),
            None => value,
        });
        self.values.push(value);
    }

    fn mean(&self) -> Option<f64> {
        if self.count > 0 {
            Some(self.sum / self.count as f64)
        } else {
            None
        }
    }

    fn median(&self) -> Option<f64> {
        if self.values.is_empty() {
            return None;
        }
        let mut sorted = self.values.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let mid = sorted.len() / 2;
        if sorted.len().is_multiple_of(2) {
            Some((sorted[mid - 1] + sorted[mid]) / 2.0)
        } else {
            Some(sorted[mid])
        }
    }

    fn std_dev(&self) -> Option<f64> {
        if self.count < 2 {
            return None;
        }
        let mean = self.mean()?;
        let variance =
            (self.sum_squares - self.count as f64 * mean * mean) / (self.count as f64 - 1.0);
        Some(variance.max(0.0).sqrt())
    }
}

fn format_number(value: f64) -> String {
    if value.fract() == 0.0 {
        format!("{value:.0}")
    } else {
        format!("{value:.4}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use encoding_rs::UTF_8;

    fn fixture_path() -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("data")
            .join("ipqs_nonfraud_signaldata.tsv")
    }

    #[test]
    fn accumulator_computes_stats_for_ipqs_subset() {
        let path = fixture_path();
        assert!(path.exists(), "fixture missing: {path:?}");
        let schema = crate::schema::infer_schema(&path, 200, b'\t', UTF_8).expect("infer schema");
        let columns = vec![
            schema
                .column_index("ipqs_email_Fraud Score")
                .expect("email score index"),
            schema
                .column_index("ipqs_phone_Fraud Score")
                .expect("phone score index"),
        ];
        let mut accumulator = StatsAccumulator::new(&columns, &schema);
        let mut reader =
            crate::io_utils::open_csv_reader_from_path(&path, b'\t', true).expect("open csv");
        crate::io_utils::reader_headers(&mut reader, UTF_8).expect("headers");

        for (idx, record) in reader.byte_records().enumerate() {
            if idx >= 100 {
                break;
            }
            let record = record.expect("record");
            let decoded = crate::io_utils::decode_record(&record, UTF_8).expect("decode");
            accumulator.ingest(&schema, &decoded).expect("ingest row");
        }

        let rows = accumulator.render_rows();
        assert_eq!(rows.len(), columns.len());
        let email_stats = rows
            .iter()
            .find(|row| row[0] == "ipqs_email_Fraud Score")
            .expect("email stats");
        assert_ne!(email_stats[1], "0");
        assert!(!email_stats[4].is_empty());
    }
}
