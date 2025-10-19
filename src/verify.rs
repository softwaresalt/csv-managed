use std::path::Path;

use anyhow::{Context, Result, anyhow};
use log::info;

use crate::{
    cli::VerifyArgs,
    data::parse_typed_value,
    io_utils,
    schema::{ColumnType, Schema},
};

pub fn execute(args: &VerifyArgs) -> Result<()> {
    let input_encoding = io_utils::resolve_encoding(args.input_encoding.as_deref())?;
    let schema = Schema::load(&args.schema)
        .with_context(|| format!("Loading schema from {schema:?}", schema = args.schema))?;
    for input in &args.inputs {
        let delimiter = io_utils::resolve_input_delimiter(input, args.delimiter);
        validate_file_against_schema(&schema, input, delimiter, input_encoding)?;
        info!("✓ {input:?} matches schema");
    }
    Ok(())
}

pub fn validate_file_against_schema(
    schema: &Schema,
    path: &Path,
    delimiter: u8,
    encoding: &'static encoding_rs::Encoding,
) -> Result<()> {
    let mut reader = io_utils::open_csv_reader_from_path(path, delimiter, true)?;
    let headers = io_utils::reader_headers(&mut reader, encoding)?;
    schema
        .validate_headers(&headers)
        .map_err(|err| anyhow!("Validating headers for {path:?}: {err}"))?;

    for (row_idx, record) in reader.byte_records().enumerate() {
        let record = record.with_context(|| format!("Reading row {} in {path:?}", row_idx + 2))?;
        let decoded = io_utils::decode_record(&record, encoding)?;
        for (col_idx, column) in schema.columns.iter().enumerate() {
            let raw_value = decoded.get(col_idx).map(|s| s.as_str()).unwrap_or("");
            let normalized = column.normalize_value(raw_value);
            if let Err(err) = validate_value(normalized.as_ref(), &column.datatype) {
                let message = if normalized.as_ref() == raw_value {
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
                        normalized.as_ref(),
                        err
                    )
                };
                return Err(anyhow!(message));
            }
        }
    }
    Ok(())
}

fn validate_value(value: &str, column_type: &ColumnType) -> Result<()> {
    if value.is_empty() {
        return Ok(());
    }
    parse_typed_value(value, column_type).map(|_| ())
}
