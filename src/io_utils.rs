//! I/O utilities for CSV reading, writing, encoding, and delimiter resolution.
//!
//! All file I/O in csv-managed flows through this module. It provides:
//!
//! - **Delimiter resolution**: extension-based auto-detection (`.csv` → comma,
//!   `.tsv` → tab) with manual override support.
//! - **Encoding**: input decoding and output transcoding via `encoding_rs`,
//!   defaulting to UTF-8.
//! - **Reader/writer construction**: `open_csv_reader`, `open_csv_writer`,
//!   and seekable reader variants for index-accelerated reads.
//! - **stdin/stdout**: the `-` path convention routes through standard streams.
//! - **Quoting**: CSV output uses `QuoteStyle::Always` for round-trip safety.

use std::{
    fs::File,
    io::{self, BufReader, BufWriter, Read, Write},
    path::Path,
};

use anyhow::{Context, Result, anyhow};
use csv::QuoteStyle;
use encoding_rs::{Encoding, UTF_8};

pub const DEFAULT_CSV_DELIMITER: u8 = b',';
pub const DEFAULT_TSV_DELIMITER: u8 = b'\t';

pub fn is_dash(path: &Path) -> bool {
    path == Path::new("-")
}

pub fn resolve_encoding(label: Option<&str>) -> Result<&'static Encoding> {
    if let Some(value) = label {
        Encoding::for_label(value.trim().as_bytes())
            .ok_or_else(|| anyhow!("Unknown encoding '{value}'"))
    } else {
        Ok(UTF_8)
    }
}

pub fn resolve_input_delimiter(path: &Path, provided: Option<u8>) -> u8 {
    provided.unwrap_or_else(|| match path.extension().and_then(|ext| ext.to_str()) {
        Some(ext) if ext.eq_ignore_ascii_case("tsv") => DEFAULT_TSV_DELIMITER,
        _ => DEFAULT_CSV_DELIMITER,
    })
}

pub fn resolve_output_delimiter(path: Option<&Path>, provided: Option<u8>, fallback: u8) -> u8 {
    if let Some(delim) = provided {
        return delim;
    }
    if let Some(path) = path {
        match path.extension().and_then(|ext| ext.to_str()) {
            Some(ext) if ext.eq_ignore_ascii_case("tsv") => return DEFAULT_TSV_DELIMITER,
            Some(ext) if ext.eq_ignore_ascii_case("csv") => return DEFAULT_CSV_DELIMITER,
            _ => {}
        }
    }
    fallback
}

pub fn open_csv_reader<R>(reader: R, delimiter: u8, has_headers: bool) -> csv::Reader<R>
where
    R: Read,
{
    let mut builder = csv::ReaderBuilder::new();
    builder
        .has_headers(has_headers)
        .delimiter(delimiter)
        .double_quote(true)
        .flexible(false);
    builder.from_reader(reader)
}

pub fn open_csv_reader_from_path(
    path: &Path,
    delimiter: u8,
    has_headers: bool,
) -> Result<csv::Reader<Box<dyn Read>>> {
    let reader: Box<dyn Read> = if is_dash(path) {
        Box::new(std::io::stdin().lock())
    } else {
        Box::new(BufReader::new(
            File::open(path).with_context(|| format!("Opening input file {path:?}"))?,
        ))
    };
    Ok(open_csv_reader(reader, delimiter, has_headers))
}

pub fn open_seekable_csv_reader(
    path: &Path,
    delimiter: u8,
    has_headers: bool,
) -> Result<csv::Reader<BufReader<File>>> {
    let reader =
        BufReader::new(File::open(path).with_context(|| format!("Opening input file {path:?}"))?);
    Ok(open_csv_reader(reader, delimiter, has_headers))
}

pub fn open_csv_writer(
    path: Option<&Path>,
    delimiter: u8,
    encoding: &'static Encoding,
) -> Result<csv::Writer<Box<dyn Write>>> {
    let base: Box<dyn Write> = match path {
        Some(p) if !is_dash(p) => Box::new(BufWriter::new(
            File::create(p).with_context(|| format!("Creating output file {p:?}"))?,
        )),
        _ => Box::new(std::io::stdout()),
    };

    let writer: Box<dyn Write> = if encoding == UTF_8 {
        base
    } else {
        Box::new(TranscodingWriter::new(base, encoding))
    };

    let mut builder = csv::WriterBuilder::new();
    builder
        .delimiter(delimiter)
        .quote_style(QuoteStyle::Always)
        .double_quote(true);
    Ok(builder.from_writer(writer))
}

pub fn decode_bytes(bytes: &[u8], encoding: &'static Encoding) -> Result<String> {
    let (text, _, had_errors) = encoding.decode(bytes);
    if had_errors {
        Err(anyhow!(
            "Failed to decode text with encoding {}",
            encoding.name()
        ))
    } else {
        Ok(text.into_owned())
    }
}

pub fn decode_record(record: &csv::ByteRecord, encoding: &'static Encoding) -> Result<Vec<String>> {
    record
        .iter()
        .map(|field| decode_bytes(field, encoding))
        .collect()
}

pub fn decode_headers(
    record: &csv::ByteRecord,
    encoding: &'static Encoding,
) -> Result<Vec<String>> {
    decode_record(record, encoding)
}

pub fn reader_headers<R>(
    reader: &mut csv::Reader<R>,
    encoding: &'static Encoding,
) -> Result<Vec<String>>
where
    R: Read,
{
    let headers = reader.byte_headers()?.clone();
    decode_headers(&headers, encoding)
}

struct TranscodingWriter<W: Write> {
    inner: W,
    encoding: &'static Encoding,
    buffer: Vec<u8>,
}

impl<W: Write> TranscodingWriter<W> {
    fn new(inner: W, encoding: &'static Encoding) -> Self {
        Self {
            inner,
            encoding,
            buffer: Vec::new(),
        }
    }

    fn flush_buffer(&mut self, force: bool) -> io::Result<()> {
        let mut idx = 0;
        while idx < self.buffer.len() {
            match std::str::from_utf8(&self.buffer[idx..]) {
                Ok(valid) => {
                    let text = valid.to_owned();
                    self.encode_and_write(&text)?;
                    self.buffer.clear();
                    return Ok(());
                }
                Err(err) => {
                    if let Some(error_len) = err.error_len() {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            format!("Invalid UTF-8 sequence in output stream ({error_len} bytes)"),
                        ));
                    }
                    let valid_up_to = err.valid_up_to();
                    if valid_up_to > 0 {
                        let valid_slice = &self.buffer[idx..idx + valid_up_to];
                        let text = std::str::from_utf8(valid_slice)
                            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?
                            .to_owned();
                        self.encode_and_write(&text)?;
                        self.buffer.drain(..idx + valid_up_to);
                        idx = 0;
                        continue;
                    }
                    if force {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            "Incomplete UTF-8 sequence at end of output stream",
                        ));
                    } else {
                        return Ok(());
                    }
                }
            }
        }
        if force && !self.buffer.is_empty() {
            let text = String::from_utf8(self.buffer.clone()).map_err(|_| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Invalid UTF-8 sequence at end of output stream",
                )
            })?;
            self.encode_and_write(&text)?;
            self.buffer.clear();
        }
        Ok(())
    }

    fn encode_and_write(&mut self, text: &str) -> io::Result<()> {
        let (encoded, _output_encoding, had_errors) = self.encoding.encode(text);
        if had_errors {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Failed to encode text using {}", self.encoding.name()),
            ));
        }
        self.inner.write_all(encoded.as_ref())
    }
}

impl<W: Write> Write for TranscodingWriter<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.buffer.extend_from_slice(buf);
        self.flush_buffer(false)?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.flush_buffer(true)?;
        self.inner.flush()
    }
}
