use std::path::Path;

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
        Some(Schema::load(path).with_context(|| format!("Loading schema from {path:?}"))?)
    } else {
        None
    };

    let mut baseline_headers: Option<Vec<String>> = None;
    let mut writer =
        io_utils::open_csv_writer(args.output.as_deref(), output_delimiter, output_encoding)?;
    let mut total_rows = 0usize;
    let context = AppendContext {
        delimiter,
        encoding: input_encoding,
        schema: schema.as_ref(),
    };

    {
        let mut state = AppendState {
            writer: &mut writer,
            baseline_headers: &mut baseline_headers,
            total_rows: &mut total_rows,
        };

        for (idx, input) in args.inputs.iter().enumerate() {
            append_single(input.as_path(), idx == 0, &context, &mut state)?;
            info!("✓ Appended {input:?}");
        }
    }

    info!("Wrote {total_rows} data row(s) to output");
    Ok(())
}

struct AppendContext<'schema> {
    delimiter: u8,
    encoding: &'static encoding_rs::Encoding,
    schema: Option<&'schema Schema>,
}

struct AppendState<'writer> {
    writer: &'writer mut csv::Writer<Box<dyn std::io::Write>>,
    baseline_headers: &'writer mut Option<Vec<String>>,
    total_rows: &'writer mut usize,
}

fn append_single(
    path: &Path,
    write_header: bool,
    context: &AppendContext<'_>,
    state: &mut AppendState<'_>,
) -> Result<()> {
    let mut reader = io_utils::open_csv_reader_from_path(path, context.delimiter, true)?;
    let headers = io_utils::reader_headers(&mut reader, context.encoding)?;

    if let Some(schema) = context.schema {
        schema
            .validate_headers(&headers)
            .with_context(|| format!("Validating headers for {path:?}"))?;
    } else if let Some(baseline) = state.baseline_headers {
        if headers.len() != baseline.len()
            || !headers
                .iter()
                .zip(baseline.iter())
                .all(|(left, right)| left == right)
        {
            return Err(anyhow!(
                "Header mismatch between {path:?} and baseline ({baseline:?})"
            ));
        }
    } else {
        *state.baseline_headers = Some(headers.clone());
    }

    if write_header {
        if let Some(schema) = context.schema {
            let output_headers = schema.output_headers();
            state
                .writer
                .write_record(output_headers.iter())
                .with_context(|| "Writing output headers")?;
        } else {
            state
                .writer
                .write_record(headers.iter())
                .with_context(|| "Writing output headers")?;
        }
    }

    for (row_idx, record) in reader.byte_records().enumerate() {
        let record = record.with_context(|| format!("Reading row {} in {path:?}", row_idx + 2))?;
        let decoded = io_utils::decode_record(&record, context.encoding)?;
        if let Some(schema) = context.schema {
            validate_record(schema, &decoded, row_idx + 2)?;
        }
        state
            .writer
            .write_record(decoded.iter())
            .with_context(|| format!("Writing row {} from {path:?}", row_idx + 2))?;
        *state.total_rows += 1;
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
