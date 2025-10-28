use std::{collections::HashMap, path::PathBuf};

use anyhow::{Context, Result, anyhow};
use encoding_rs::Encoding;
use log::info;

use crate::{
    data::parse_typed_value,
    io_utils,
    schema::{self, ColumnType, Schema},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum JoinKind {
    #[default]
    Inner,
    Left,
    Right,
    Full,
}

#[derive(Debug, Clone)]
pub struct JoinArgs {
    pub left: PathBuf,
    pub right: PathBuf,
    pub output: Option<PathBuf>,
    pub left_key: String,
    pub right_key: String,
    pub kind: JoinKind,
    pub left_schema: Option<PathBuf>,
    pub right_schema: Option<PathBuf>,
    pub delimiter: Option<u8>,
    pub left_encoding: Option<String>,
    pub right_encoding: Option<String>,
    pub output_encoding: Option<String>,
}

const KEY_SEPARATOR: &str = "\u{1f}";

pub fn execute(args: &JoinArgs) -> Result<()> {
    if args.left_key.is_empty() || args.right_key.is_empty() {
        return Err(anyhow!("Join requires --left-key and --right-key"));
    }
    if io_utils::is_dash(&args.right) {
        return Err(anyhow!(
            "Right input cannot be stdin for join operations; provide a file path"
        ));
    }
    if io_utils::is_dash(&args.left) && args.left_schema.is_none() {
        return Err(anyhow!(
            "Joining from stdin requires --left-schema (or --left-meta) to describe the schema"
        ));
    }

    let left_keys = parse_key_list(&args.left_key)?;
    let right_keys = parse_key_list(&args.right_key)?;
    if left_keys.len() != right_keys.len() {
        return Err(anyhow!(
            "Left and right join keys must contain the same number of columns"
        ));
    }

    let left_delimiter = io_utils::resolve_input_delimiter(&args.left, args.delimiter);
    let right_delimiter = io_utils::resolve_input_delimiter(&args.right, args.delimiter);
    let output_delimiter =
        io_utils::resolve_output_delimiter(args.output.as_deref(), None, left_delimiter);
    let left_encoding = io_utils::resolve_encoding(args.left_encoding.as_deref())?;
    let right_encoding = io_utils::resolve_encoding(args.right_encoding.as_deref())?;
    let output_encoding = io_utils::resolve_encoding(args.output_encoding.as_deref())?;

    let left_schema = load_schema(
        &args.left,
        args.left_schema.as_ref(),
        left_delimiter,
        left_encoding,
    )?;
    let right_schema = load_schema(
        &args.right,
        args.right_schema.as_ref(),
        right_delimiter,
        right_encoding,
    )?;

    let left_indices = column_indices(&left_schema, &left_keys)?;
    let right_indices = column_indices(&right_schema, &right_keys)?;
    validate_key_types(&left_schema, &right_schema, &left_indices, &right_indices)?;

    let left_expects_headers = left_schema.expects_headers();
    let right_expects_headers = right_schema.expects_headers();

    let mut left_reader =
        io_utils::open_csv_reader_from_path(&args.left, left_delimiter, left_expects_headers)?;
    let mut right_reader =
        io_utils::open_csv_reader_from_path(&args.right, right_delimiter, right_expects_headers)?;

    let left_headers = if left_expects_headers {
        let headers = io_utils::reader_headers(&mut left_reader, left_encoding)?;
        left_schema
            .validate_headers(&headers)
            .with_context(|| format!("Validating left headers for {:?}", args.left))?;
        headers
    } else {
        left_schema.headers()
    };

    let right_headers = if right_expects_headers {
        let headers = io_utils::reader_headers(&mut right_reader, right_encoding)?;
        right_schema
            .validate_headers(&headers)
            .with_context(|| format!("Validating right headers for {:?}", args.right))?;
        headers
    } else {
        right_schema.headers()
    };

    let mut right_lookup = build_right_lookup(
        &mut right_reader,
        &right_schema,
        &right_indices,
        right_encoding,
    )?;

    let (output_headers, right_columns) =
        build_output_headers(&left_headers, &right_headers, &right_indices);

    let mut writer =
        io_utils::open_csv_writer(args.output.as_deref(), output_delimiter, output_encoding)?;
    writer
        .write_record(&output_headers)
        .context("Writing joined headers")?;

    let mut output_rows = 0usize;
    let mut matched_rows = 0usize;
    let include_unmatched_left = matches!(args.kind, JoinKind::Left | JoinKind::Full);
    let include_unmatched_right = matches!(args.kind, JoinKind::Right | JoinKind::Full);
    let key_pairs: Vec<(usize, usize)> = left_indices
        .iter()
        .cloned()
        .zip(right_indices.iter().cloned())
        .collect();

    for (row_idx, record) in left_reader.byte_records().enumerate() {
        let record = record.with_context(|| format!("Reading left row {}", row_idx + 2))?;
        let mut decoded = io_utils::decode_record(&record, left_encoding)?;
        if left_schema.has_transformations() {
            left_schema
                .apply_transformations_to_row(&mut decoded)
                .with_context(|| {
                    format!(
                        "Applying datatype mappings to left row {} in {:?}",
                        row_idx + 2,
                        args.left
                    )
                })?;
        }
        left_schema.apply_replacements_to_row(&mut decoded);
        let key = build_key(&decoded, &left_schema, &left_indices)?;
        let mut matched_any = false;
        if let Some(bucket) = right_lookup.get_mut(&key) {
            for entry in bucket.iter_mut() {
                matched_any = true;
                entry.matched = true;
                matched_rows += 1;
                let mut combined = decoded.clone();
                combined.extend(
                    right_columns
                        .iter()
                        .map(|idx| entry.record.get(*idx).cloned().unwrap_or_default()),
                );
                writer
                    .write_record(&combined)
                    .context("Writing joined row")?;
                output_rows += 1;
            }
        }

        if !matched_any && include_unmatched_left {
            let mut combined = decoded.clone();
            combined.extend(right_columns.iter().map(|_| String::new()));
            writer
                .write_record(&combined)
                .context("Writing left outer row")?;
            output_rows += 1;
        }
    }

    if include_unmatched_right {
        for bucket in right_lookup.values() {
            for entry in bucket.iter() {
                if entry.matched {
                    continue;
                }
                let mut left_part = vec![String::new(); left_headers.len()];
                for (left_idx, right_idx) in &key_pairs {
                    let value = entry.record.get(*right_idx).cloned().unwrap_or_default();
                    left_part[*left_idx] = value;
                }
                let mut combined = left_part;
                combined.extend(
                    right_columns
                        .iter()
                        .map(|idx| entry.record.get(*idx).cloned().unwrap_or_default()),
                );
                writer
                    .write_record(&combined)
                    .context("Writing right outer row")?;
                output_rows += 1;
            }
        }
    }

    writer.flush().context("Flushing join output")?;
    info!("Join complete: {output_rows} output row(s), {matched_rows} matched row(s)");
    Ok(())
}

fn parse_key_list(value: &str) -> Result<Vec<String>> {
    let parts = value
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect::<Vec<_>>();
    if parts.is_empty() {
        Err(anyhow!("Join key list cannot be empty"))
    } else {
        Ok(parts)
    }
}

fn load_schema(
    path: &PathBuf,
    schema_path: Option<&PathBuf>,
    delimiter: u8,
    encoding: &'static Encoding,
) -> Result<Schema> {
    if let Some(schema_path) = schema_path {
        Schema::load(schema_path).with_context(|| format!("Loading schema from {schema_path:?}"))
    } else {
        schema::infer_schema(path, 0, delimiter, encoding, None)
            .with_context(|| format!("Inferring schema from {path:?}"))
    }
}

fn column_indices(schema: &Schema, columns: &[String]) -> Result<Vec<usize>> {
    columns
        .iter()
        .map(|name| {
            schema
                .column_index(name)
                .ok_or_else(|| anyhow!("Column '{name}' not found in schema"))
        })
        .collect()
}

fn validate_key_types(
    left_schema: &Schema,
    right_schema: &Schema,
    left_indices: &[usize],
    right_indices: &[usize],
) -> Result<()> {
    for (l_idx, r_idx) in left_indices.iter().zip(right_indices.iter()) {
        let left_type = &left_schema.columns[*l_idx].datatype;
        let right_type = &right_schema.columns[*r_idx].datatype;
        if !same_type(left_type, right_type) {
            return Err(anyhow!(
                "Type mismatch for join keys: left {left_type:?} vs right {right_type:?}"
            ));
        }
    }
    Ok(())
}

fn same_type(left: &ColumnType, right: &ColumnType) -> bool {
    match (left, right) {
        (ColumnType::Integer, ColumnType::Float) | (ColumnType::Float, ColumnType::Integer) => true,
        _ => left == right,
    }
}

struct RightRow {
    record: Vec<String>,
    matched: bool,
}

fn build_right_lookup(
    reader: &mut csv::Reader<Box<dyn std::io::Read>>,
    schema: &Schema,
    key_indices: &[usize],
    encoding: &'static Encoding,
) -> Result<HashMap<String, Vec<RightRow>>> {
    let mut map: HashMap<String, Vec<RightRow>> = HashMap::new();
    for (row_idx, record) in reader.byte_records().enumerate() {
        let record = record.with_context(|| format!("Reading right row {}", row_idx + 2))?;
        let mut decoded = io_utils::decode_record(&record, encoding)?;
        if schema.has_transformations() {
            schema
                .apply_transformations_to_row(&mut decoded)
                .with_context(|| {
                    format!("Applying datatype mappings to right row {}", row_idx + 2)
                })?;
        }
        schema.apply_replacements_to_row(&mut decoded);
        let key = build_key(&decoded, schema, key_indices)?;
        map.entry(key).or_default().push(RightRow {
            record: decoded,
            matched: false,
        });
    }
    Ok(map)
}

fn build_key(record: &[String], schema: &Schema, key_indices: &[usize]) -> Result<String> {
    let mut parts = Vec::with_capacity(key_indices.len());
    for idx in key_indices {
        let column = &schema.columns[*idx];
        let raw = record.get(*idx).map(|s| s.as_str()).unwrap_or("");
        let normalized = column.normalize_value(raw);
        let parsed = parse_typed_value(normalized.as_ref(), &column.datatype)
            .with_context(|| format!("Parsing join key for column '{}'", column.name))?;
        if let Some(value) = parsed {
            parts.push(value.as_display());
        } else {
            parts.push(String::new());
        }
    }
    Ok(parts.join(KEY_SEPARATOR))
}

fn build_output_headers(
    left_headers: &[String],
    right_headers: &[String],
    right_key_indices: &[usize],
) -> (Vec<String>, Vec<usize>) {
    use std::collections::HashSet;

    let mut headers = left_headers.to_vec();
    let mut seen: HashSet<String> = headers.iter().cloned().collect();
    let mut right_columns = Vec::new();

    for (idx, name) in right_headers.iter().enumerate() {
        if right_key_indices.contains(&idx) {
            continue;
        }
        let mut candidate = name.clone();
        if seen.contains(&candidate) {
            let mut counter = 1usize;
            let base = candidate.clone();
            while seen.contains(&candidate) {
                candidate = format!("right_{base}_{counter}");
                counter += 1;
            }
        }
        seen.insert(candidate.clone());
        headers.push(candidate);
        right_columns.push(idx);
    }

    (headers, right_columns)
}
