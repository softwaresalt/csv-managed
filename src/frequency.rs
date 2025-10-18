use std::collections::HashMap;

use anyhow::{Context, Result, anyhow};
use encoding_rs::Encoding;
use log::info;

use crate::{
    cli::FrequencyArgs,
    data::{Value, parse_typed_value},
    io_utils,
    schema::{self, Schema},
    table,
};

pub fn execute(args: &FrequencyArgs) -> Result<()> {
    if args.schema.is_none() && io_utils::is_dash(&args.input) {
        return Err(anyhow!(
            "Reading from stdin requires --schema (or --meta) for frequency analysis"
        ));
    }

    let delimiter = io_utils::resolve_input_delimiter(&args.input, args.delimiter);
    let encoding = io_utils::resolve_encoding(args.input_encoding.as_deref())?;

    let schema = load_or_infer_schema(args, delimiter, encoding)?;
    let columns = resolve_columns(&schema, &args.columns)?;
    if columns.is_empty() {
        return Err(anyhow!(
            "No columns available for frequency analysis. Supply --columns to continue."
        ));
    }

    let mut reader = io_utils::open_csv_reader_from_path(&args.input, delimiter, true)?;
    let headers = io_utils::reader_headers(&mut reader, encoding)?;
    schema
        .validate_headers(&headers)
        .with_context(|| format!("Validating headers for {:?}", args.input))?;

    let mut stats = FrequencyAccumulator::new(&columns, &schema);

    for (row_idx, record) in reader.byte_records().enumerate() {
        let record = record.with_context(|| format!("Reading row {}", row_idx + 2))?;
        let decoded = io_utils::decode_record(&record, encoding)?;
        stats
            .ingest(&schema, &decoded)
            .with_context(|| format!("Processing row {}", row_idx + 2))?;
    }

    let mut rows = Vec::new();
    for column_index in &columns {
        rows.extend(stats.render_rows(*column_index, args.top));
    }

    let headers = vec![
        "column".to_string(),
        "value".to_string(),
        "count".to_string(),
        "percent".to_string(),
    ];
    table::print_table(&headers, &rows);
    info!("Computed frequency counts for {} column(s)", columns.len());
    Ok(())
}

fn load_or_infer_schema(
    args: &FrequencyArgs,
    delimiter: u8,
    encoding: &'static Encoding,
) -> Result<Schema> {
    if let Some(path) = &args.schema {
        Schema::load(path).with_context(|| format!("Loading schema from {:?}", path))
    } else {
        schema::infer_schema(&args.input, 0, delimiter, encoding)
            .with_context(|| format!("Inferring schema from {:?}", args.input))
    }
}

fn resolve_columns(schema: &Schema, specified: &[String]) -> Result<Vec<usize>> {
    if specified.is_empty() {
        Ok((0..schema.columns.len()).collect())
    } else {
        specified
            .iter()
            .map(|name| {
                schema
                    .column_index(name)
                    .ok_or_else(|| anyhow!("Column '{name}' not found in schema"))
            })
            .collect()
    }
}

struct FrequencyAccumulator {
    columns: Vec<usize>,
    totals: HashMap<usize, usize>,
    counts: HashMap<usize, HashMap<String, usize>>,
    names: HashMap<usize, String>,
}

impl FrequencyAccumulator {
    fn new(columns: &[usize], schema: &Schema) -> Self {
        let mut totals = HashMap::new();
        let mut counts = HashMap::new();
        let mut names = HashMap::new();
        for idx in columns {
            totals.insert(*idx, 0);
            counts.insert(*idx, HashMap::new());
            names.insert(*idx, schema.columns[*idx].output_name().to_string());
        }
        Self {
            columns: columns.to_vec(),
            totals,
            counts,
            names,
        }
    }

    fn ingest(&mut self, schema: &Schema, record: &[String]) -> Result<()> {
        for column_index in &self.columns {
            let column = &schema.columns[*column_index];
            let raw = record.get(*column_index).map(|s| s.as_str()).unwrap_or("");
            let value = if raw.is_empty() {
                String::from("<empty>")
            } else {
                match parse_typed_value(raw, &column.data_type)
                    .with_context(|| format!("Column '{}'", column.output_name()))?
                {
                    Some(Value::String(s)) => s,
                    Some(Value::Integer(i)) => i.to_string(),
                    Some(Value::Float(f)) => format_number(f),
                    Some(Value::Boolean(b)) => b.to_string(),
                    Some(Value::Date(d)) => d.format("%Y-%m-%d").to_string(),
                    Some(Value::DateTime(dt)) => dt.format("%Y-%m-%d %H:%M:%S").to_string(),
                    Some(Value::Guid(g)) => g.to_string(),
                    None => String::from("<empty>"),
                }
            };
            let total = self
                .totals
                .get_mut(column_index)
                .expect("Column should exist in totals");
            *total += 1;
            let counter = self
                .counts
                .get_mut(column_index)
                .expect("Column should exist in counts");
            *counter.entry(value).or_insert(0) += 1;
        }
        Ok(())
    }

    fn render_rows(&self, column_index: usize, top: usize) -> Vec<Vec<String>> {
        let total = match self.totals.get(&column_index) {
            Some(total) if *total > 0 => *total,
            _ => return Vec::new(),
        };
        let mut items = self
            .counts
            .get(&column_index)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .collect::<Vec<_>>();
        items.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        if top > 0 && items.len() > top {
            items.truncate(top);
        }
        items
            .into_iter()
            .map(|(value, count)| {
                let percent = (count as f64 / total as f64) * 100.0;
                vec![
                    self.names
                        .get(&column_index)
                        .cloned()
                        .unwrap_or_else(|| column_index.to_string()),
                    value,
                    count.to_string(),
                    format!("{percent:.2}%"),
                ]
            })
            .collect()
    }
}

fn format_number(value: f64) -> String {
    if value.fract() == 0.0 {
        format!("{:.0}", value)
    } else {
        format!("{:.4}", value)
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
    fn accumulator_counts_ipqs_boolean_values() {
        let path = fixture_path();
        assert!(path.exists(), "fixture missing: {:?}", path);
        let schema = crate::schema::infer_schema(&path, 200, b'\t', UTF_8).expect("infer schema");
        let column_index = schema
            .column_index("ipqs_email_Valid")
            .expect("column index");
        let mut accumulator = FrequencyAccumulator::new(&[column_index], &schema);
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

        let rows = accumulator.render_rows(column_index, 3);
        assert!(!rows.is_empty());
        assert_eq!(rows[0][0], "ipqs_email_Valid");
    }
}
