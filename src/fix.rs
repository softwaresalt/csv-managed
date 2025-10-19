use anyhow::{Context, Result};
use log::info;

use crate::{cli::FixArgs, io_utils, schema::Schema};

pub fn execute(args: &FixArgs) -> Result<()> {
    let input_delimiter = io_utils::resolve_input_delimiter(&args.input, args.delimiter);
    let input_encoding = io_utils::resolve_encoding(args.input_encoding.as_deref())?;
    let output_encoding = io_utils::resolve_encoding(args.output_encoding.as_deref())?;

    let schema = Schema::load(&args.schema)
        .with_context(|| format!("Loading schema from {:?}", args.schema))?;

    let mut reader = io_utils::open_csv_reader_from_path(&args.input, input_delimiter, true)?;
    let headers = io_utils::reader_headers(&mut reader, input_encoding)?;
    schema
        .validate_headers(&headers)
        .context("Validating input headers against schema")?;

    let output_delimiter = io_utils::resolve_output_delimiter(
        args.output.as_deref(),
        args.output_delimiter,
        input_delimiter,
    );
    let mut writer =
        io_utils::open_csv_writer(args.output.as_deref(), output_delimiter, output_encoding)?;
    writer
        .write_record(headers.iter())
        .context("Writing output headers")?;

    let mut rows = 0usize;
    for (idx, record) in reader.byte_records().enumerate() {
        let record = record.with_context(|| format!("Reading row {}", idx + 2))?;
        let mut values = io_utils::decode_record(&record, input_encoding)?;
        schema.apply_replacements_to_row(&mut values);
        writer
            .write_record(values.iter())
            .with_context(|| format!("Writing output row {}", idx + 2))?;
        rows += 1;
    }
    writer.flush().context("Flushing output writer")?;

    let destination = args
        .output
        .as_ref()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| "stdout".to_string());
    let replacements: usize = schema
        .columns
        .iter()
        .map(|column| column.value_replacements.len())
        .sum();
    info!(
        "Applied {} replacement mapping(s) across {} row(s) -> {}",
        replacements, rows, destination
    );

    Ok(())
}
