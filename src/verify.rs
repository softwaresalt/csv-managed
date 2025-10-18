use std::path::Path;

use anyhow::{Context, Result};
use log::info;

use crate::{
    cli::VerifyArgs,
    data::parse_typed_value,
    io_utils,
    metadata::{ColumnType, Schema},
};

pub fn execute(args: &VerifyArgs) -> Result<()> {
    let input_encoding = io_utils::resolve_encoding(args.input_encoding.as_deref())?;
    let schema = Schema::load(&args.meta)
        .with_context(|| format!("Loading metadata from {:?}", args.meta))?;
    for input in &args.inputs {
        let delimiter = io_utils::resolve_input_delimiter(input, args.delimiter);
        validate_file_against_schema(&schema, input, delimiter, input_encoding)?;
        info!("✓ {:?} matches schema", input);
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
        .with_context(|| format!("Validating headers for {:?}", path))?;

    for (row_idx, record) in reader.byte_records().enumerate() {
        let record =
            record.with_context(|| format!("Reading row {} in {:?}", row_idx + 2, path))?;
        let decoded = io_utils::decode_record(&record, encoding)?;
        for (col_idx, column) in schema.columns.iter().enumerate() {
            let value = decoded.get(col_idx).map(|s| s.as_str()).unwrap_or("");
            validate_value(value, &column.data_type)
                .with_context(|| format!("Row {} column '{}'", row_idx + 2, column.name))?;
        }
    }
    Ok(())
}

fn validate_value(value: &str, column_type: &ColumnType) -> Result<()> {
    if value.is_empty() {
        return Ok(());
    }
    // parse_typed_value returns Option<Value>, we only care about success/failure
    parse_typed_value(value, column_type).map(|_| ())
}
