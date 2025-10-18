use anyhow::{Context, Result};
use log::info;

use crate::{cli::PreviewArgs, io_utils, table};

pub fn execute(args: &PreviewArgs) -> Result<()> {
    let delimiter = io_utils::resolve_input_delimiter(&args.input, args.delimiter);
    let encoding = io_utils::resolve_encoding(args.input_encoding.as_deref())?;
    let mut reader = io_utils::open_csv_reader_from_path(&args.input, delimiter, true)?;
    let headers = io_utils::reader_headers(&mut reader, encoding)?;
    let mut rows = Vec::new();

    for (idx, record) in reader.byte_records().enumerate() {
        if idx >= args.rows {
            break;
        }
        let record = record.with_context(|| format!("Reading row {}", idx + 2))?;
        let decoded = io_utils::decode_record(&record, encoding)?;
        rows.push(decoded);
    }

    table::print_table(&headers, &rows);
    info!("Displayed {} row(s) from {:?}", rows.len(), args.input);
    Ok(())
}
