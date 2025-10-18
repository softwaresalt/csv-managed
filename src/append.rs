use std::path::PathBuf;

use anyhow::{Context, Result, anyhow};
use log::info;

use crate::{cli::AppendArgs, data::parse_typed_value, io_utils, schema::Schema};

pub fn execute(args: &AppendArgs) -> Result<()> {
    if args.inputs.is_empty() {
        return Err(anyhow!("At least one input file must be provided"));
    }

    let delimiter = io_utils::resolve_input_delimiter(&args.inputs[0], args.delimiter);
    let input_encoding = io_utils::resolve_encoding(args.input_encoding.as_deref())?;
    let output_delimiter =
        io_utils::resolve_output_delimiter(args.output.as_deref(), None, delimiter);
    let output_encoding = io_utils::resolve_encoding(args.output_encoding.as_deref())?;

    let schema = if let Some(path) = &args.schema {
        Some(Schema::load(path).with_context(|| format!("Loading schema from {:?}", path))?)
    } else {
        None
    };

    let mut baseline_headers: Option<Vec<String>> = None;
    let mut writer =
        io_utils::open_csv_writer(args.output.as_deref(), output_delimiter, output_encoding)?;
    let mut total_rows = 0usize;

    for (idx, input) in args.inputs.iter().enumerate() {
        append_single(
            input,
            delimiter,
            input_encoding,
            idx == 0,
            &mut writer,
            &mut baseline_headers,
            schema.as_ref(),
            &mut total_rows,
        )?;
        info!("✓ Appended {:?}", input);
    }

    info!("Wrote {total_rows} data row(s) to output");
    Ok(())
}

fn append_single(
    path: &PathBuf,
    delimiter: u8,
    encoding: &'static encoding_rs::Encoding,
    write_header: bool,
    writer: &mut csv::Writer<Box<dyn std::io::Write>>,
    baseline_headers: &mut Option<Vec<String>>,
    schema: Option<&Schema>,
    total_rows: &mut usize,
) -> Result<()> {
    let mut reader = io_utils::open_csv_reader_from_path(path, delimiter, true)?;
    let headers = io_utils::reader_headers(&mut reader, encoding)?;

    if let Some(schema) = schema {
        schema
            .validate_headers(&headers)
            .with_context(|| format!("Validating headers for {:?}", path))?;
    } else if let Some(baseline) = baseline_headers {
        if headers.len() != baseline.len()
            || !headers
                .iter()
                .zip(baseline.iter())
                .all(|(left, right)| left == right)
        {
            return Err(anyhow!(
                "Header mismatch between {:?} and baseline ({:?})",
                path,
                baseline
            ));
        }
    } else {
        *baseline_headers = Some(headers.clone());
    }

    if write_header {
        if let Some(schema) = schema {
            let output_headers = schema.output_headers();
            writer
                .write_record(output_headers.iter())
                .with_context(|| "Writing output headers")?;
        } else {
            writer
                .write_record(headers.iter())
                .with_context(|| "Writing output headers")?;
        }
    }

    for (row_idx, record) in reader.byte_records().enumerate() {
        let record =
            record.with_context(|| format!("Reading row {} in {:?}", row_idx + 2, path))?;
        let decoded = io_utils::decode_record(&record, encoding)?;
        if let Some(schema) = schema {
            validate_record(schema, &decoded, row_idx + 2)?;
        }
        writer
            .write_record(decoded.iter())
            .with_context(|| format!("Writing row {} from {:?}", row_idx + 2, path))?;
        *total_rows += 1;
    }

    Ok(())
}

fn validate_record(schema: &Schema, record: &[String], row_index: usize) -> Result<()> {
    for (idx, column) in schema.columns.iter().enumerate() {
        let value = record.get(idx).map(|s| s.as_str()).unwrap_or("");
        if value.is_empty() {
            continue;
        }
        parse_typed_value(value, &column.data_type)
            .with_context(|| format!("Row {row_index} column '{}'", column.output_name()))?;
    }
    Ok(())
}
