use std::{collections::HashMap, path::Path};

use anyhow::{Context, Result};
use encoding_rs::Encoding;

use crate::{
    data::Value,
    filter::{FilterCondition, evaluate_conditions},
    io_utils,
    rows::{evaluate_filter_expressions, parse_typed_row},
    schema::Schema,
};

pub struct FrequencyOptions<'a> {
    pub top: usize,
    pub row_limit: Option<usize>,
    pub filters: &'a [FilterCondition],
    pub filter_exprs: &'a [String],
}

pub fn compute_frequency_rows(
    input: &Path,
    schema: &Schema,
    delimiter: u8,
    encoding: &'static Encoding,
    columns: &[usize],
    options: &FrequencyOptions,
) -> Result<Vec<Vec<String>>> {
    let mut reader = io_utils::open_csv_reader_from_path(input, delimiter, true)?;
    let headers = io_utils::reader_headers(&mut reader, encoding)?;
    schema
        .validate_headers(&headers)
        .with_context(|| format!("Validating headers for {input:?}", input = input))?;

    let mut stats = FrequencyAccumulator::new(columns, schema);

    for (row_idx, record) in reader.byte_records().enumerate() {
        if let Some(limit) = options.row_limit
            && row_idx >= limit
        {
            break;
        }
        let record = record.with_context(|| format!("Reading row {}", row_idx + 2))?;
        let mut decoded = io_utils::decode_record(&record, encoding)?;
        if schema.has_transformations() {
            schema
                .apply_transformations_to_row(&mut decoded)
                .with_context(|| {
                    format!(
                        "Applying datatype mappings to row {} in {input:?}",
                        row_idx + 2
                    )
                })?;
        }
        schema.apply_replacements_to_row(&mut decoded);
        let typed = parse_typed_row(schema, &decoded)?;
        if !options.filters.is_empty()
            && !evaluate_conditions(options.filters, schema, &headers, &decoded, &typed)?
        {
            continue;
        }
        if !options.filter_exprs.is_empty()
            && !evaluate_filter_expressions(
                options.filter_exprs,
                &headers,
                &decoded,
                &typed,
                Some(row_idx + 1),
            )?
        {
            continue;
        }
        stats
            .ingest(schema, &decoded, &typed)
            .with_context(|| format!("Processing row {}", row_idx + 2))?;
    }

    let mut rows = Vec::new();
    for &column_index in columns {
        rows.extend(stats.render_rows(column_index, options.top));
    }
    Ok(rows)
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

    fn ingest(
        &mut self,
        schema: &Schema,
        raw_row: &[String],
        typed_row: &[Option<Value>],
    ) -> Result<()> {
        for column_index in &self.columns {
            let column = &schema.columns[*column_index];
            let raw = raw_row.get(*column_index).map(|s| s.as_str()).unwrap_or("");
            let normalized = column.normalize_value(raw);
            let value = if normalized.is_empty() {
                String::from("<empty>")
            } else if let Some(typed) = typed_row.get(*column_index).and_then(|v| v.as_ref()) {
                display_value(typed)
            } else {
                normalized.into_owned()
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

fn display_value(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Integer(i) => i.to_string(),
        Value::Float(f) => format_number(*f),
        Value::Boolean(b) => b.to_string(),
        Value::Date(d) => d.format("%Y-%m-%d").to_string(),
        Value::DateTime(dt) => dt.format("%Y-%m-%d %H:%M:%S").to_string(),
        Value::Time(t) => t.format("%H:%M:%S").to_string(),
        Value::Guid(g) => g.to_string(),
        Value::Currency(c) => c.to_string_fixed(),
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

    const DATA_FILE: &str = "big_5_players_stats_2023_2024.csv";
    const GOALS_COL: &str = "Performance_Gls";

    fn fixture_path() -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("data")
            .join(DATA_FILE)
    }

    #[test]
    fn accumulator_counts_goal_totals() {
        let path = fixture_path();
        assert!(path.exists(), "fixture missing: {path:?}");
        let delimiter = crate::io_utils::resolve_input_delimiter(&path, None);
        let schema =
            crate::schema::infer_schema(&path, 200, delimiter, UTF_8).expect("infer schema");
        let column_index = schema.column_index(GOALS_COL).expect("column index");
        let mut accumulator = FrequencyAccumulator::new(&[column_index], &schema);
        let mut reader =
            crate::io_utils::open_csv_reader_from_path(&path, delimiter, true).expect("open csv");
        crate::io_utils::reader_headers(&mut reader, UTF_8).expect("headers");

        for (idx, record) in reader.byte_records().enumerate() {
            if idx >= 100 {
                break;
            }
            let record = record.expect("record");
            let mut decoded = crate::io_utils::decode_record(&record, UTF_8).expect("decode");
            schema.apply_replacements_to_row(&mut decoded);
            let typed = crate::rows::parse_typed_row(&schema, &decoded).expect("parse typed row");
            accumulator
                .ingest(&schema, &decoded, &typed)
                .expect("ingest row");
        }

        let rows = accumulator.render_rows(column_index, 3);
        assert!(!rows.is_empty());
        assert_eq!(rows[0][0], GOALS_COL);
    }
}
