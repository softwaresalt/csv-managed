use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(author, version, about = "Manage CSV files efficiently", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Probe a CSV file and infer column data types into a .meta file
    Probe(ProbeArgs),
    /// Create a B-Tree index (.idx) for one or more columns
    Index(IndexArgs),
    /// Transform a CSV file using sorting, filtering, projection, and derivations
    Process(ProcessArgs),
}

#[derive(Debug, Args)]
pub struct ProbeArgs {
    /// Input CSV file to inspect
    #[arg(short = 'i', long = "input")]
    pub input: PathBuf,
    /// Destination .meta file path
    #[arg(short, long)]
    pub meta: PathBuf,
    /// Number of rows to sample when inferring types (0 means full scan)
    #[arg(long, default_value_t = 2000)]
    pub sample_rows: usize,
    /// CSV delimiter character (supports ',', 'tab', ';', '|')
    #[arg(long, default_value = ",", value_parser = parse_delimiter)]
    pub delimiter: u8,
}

#[derive(Debug, Args)]
pub struct IndexArgs {
    /// Input CSV file to index
    #[arg(short, long)]
    pub input: PathBuf,
    /// Output index file (.idx)
    #[arg(short = 'o', long = "index")]
    pub index: PathBuf,
    /// Columns to include in the index, in order of preference
    #[arg(short = 'C', long = "columns", required = true, value_delimiter = ',')]
    pub columns: Vec<String>,
    /// Optional metadata file describing column types
    #[arg(short, long)]
    pub meta: Option<PathBuf>,
    /// Limit number of rows to scan (useful for prototyping)
    #[arg(long)]
    pub limit: Option<usize>,
    /// CSV delimiter character (supports ',', 'tab', ';', '|')
    #[arg(long, default_value = ",", value_parser = parse_delimiter)]
    pub delimiter: u8,
}

#[derive(Debug, Args)]
pub struct ProcessArgs {
    /// Input CSV file to process
    #[arg(short = 'i', long = "input")]
    pub input: PathBuf,
    /// Output CSV file (stdout if omitted)
    #[arg(short = 'o', long = "output")]
    pub output: Option<PathBuf>,
    /// Metadata file to drive typed operations
    #[arg(short, long)]
    pub meta: Option<PathBuf>,
    /// Existing index file to speed up operations
    #[arg(short = 'x', long = "index")]
    pub index: Option<PathBuf>,
    /// Sort directives of the form `column[:asc|desc]`
    #[arg(long = "sort", action = clap::ArgAction::Append)]
    pub sort: Vec<String>,
    /// Restrict output to this comma-separated list of columns
    #[arg(short = 'C', long = "columns", action = clap::ArgAction::Append)]
    pub columns: Vec<String>,
    /// Additional derived columns using `name=expression`
    #[arg(long = "derive", action = clap::ArgAction::Append)]
    pub derives: Vec<String>,
    /// Row-level filters such as `amount>=100` or `status = shipped`
    #[arg(long = "filter", action = clap::ArgAction::Append)]
    pub filters: Vec<String>,
    /// Emit 1-based row numbers as the first column
    #[arg(long = "row-numbers")]
    pub row_numbers: bool,
    /// Limit number of rows emitted
    #[arg(long)]
    pub limit: Option<usize>,
    /// CSV delimiter character for reading input
    #[arg(long, default_value = ",", value_parser = parse_delimiter)]
    pub delimiter: u8,
    /// Delimiter to use for output (defaults to input delimiter)
    #[arg(long = "output-delimiter", value_parser = parse_delimiter)]
    pub output_delimiter: Option<u8>,
}

pub fn parse_delimiter(value: &str) -> Result<u8, String> {
    match value {
        "tab" | "\t" => Ok(b'\t'),
        "comma" | "," => Ok(b','),
        "|" | "pipe" => Ok(b'|'),
        ";" | "semicolon" => Ok(b';'),
        other => {
            let mut chars = other.chars();
            let first = chars
                .next()
                .ok_or_else(|| "Delimiter cannot be empty".to_string())?;
            if chars.next().is_some() {
                return Err("Delimiter must be a single character".to_string());
            }
            if !first.is_ascii() {
                return Err("Delimiter must be ASCII".to_string());
            }
            Ok(first as u8)
        }
    }
}
