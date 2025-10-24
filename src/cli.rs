use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

#[derive(Debug, Parser)]
#[command(author, version, about = "Manage CSV files efficiently", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Create a .schema file from explicit column definitions
    Schema(SchemaArgs),
    /// Create a B-Tree index (.idx) for one or more columns
    Index(IndexArgs),
    /// Transform a CSV file using sorting, filtering, projection, derivations, and schema-driven replacements
    Process(ProcessArgs),
    /// Append multiple CSV files into a single output
    Append(AppendArgs),
    /// Produce summary statistics for numeric columns or frequency counts via --frequency
    Stats(StatsArgs),
    // /// Join two CSV files on common columns
    // Join(JoinArgs),
    /// Install the csv-managed binary via cargo install
    Install(InstallArgs),
}

#[derive(Debug, Args)]
pub struct SchemaArgs {
    /// Manual schema creation and shared options
    #[command(subcommand)]
    pub mode: Option<SchemaMode>,
    /// Destination .schema file path (alias --schema retained for compatibility)
    #[arg(short = 'o', long = "output", alias = "schema", short_alias = 'm')]
    pub output: Option<PathBuf>,
    /// Column definitions using `name:type` syntax (comma-separated or repeatable)
    #[arg(short = 'c', long = "column", action = clap::ArgAction::Append)]
    pub columns: Vec<String>,
    /// Value replacement directives using `column=value->replacement`
    #[arg(long = "replace", action = clap::ArgAction::Append)]
    pub replacements: Vec<String>,
}

#[derive(Debug, Subcommand)]
pub enum SchemaMode {
    /// Display inferred schema details without writing a file
    Probe(SchemaProbeArgs),
    /// Infer schema metadata and optionally persist a .schema file
    Infer(SchemaInferArgs),
    /// Verify CSV files against a schema definition
    Verify(SchemaVerifyArgs),
    /// List column names and data types from a schema file
    Columns(SchemaColumnsArgs),
}

#[derive(Debug, Args, Clone)]
pub struct SchemaProbeArgs {
    /// Input CSV file to inspect
    #[arg(short = 'i', long = "input")]
    pub input: PathBuf,
    /// Number of rows to sample when inferring types (0 means full scan)
    #[arg(long = "sample-rows", default_value_t = 2000)]
    pub sample_rows: usize,
    /// CSV delimiter character (supports ',', 'tab', ';', '|')
    #[arg(long, value_parser = parse_delimiter)]
    pub delimiter: Option<u8>,
    /// Character encoding of the input file (defaults to utf-8)
    #[arg(long = "input-encoding")]
    pub input_encoding: Option<String>,
    /// Emit column mapping templates to stdout after probing
    #[arg(long = "mapping")]
    pub mapping: bool,
    /// Override inferred column types using `name:type`
    #[arg(long = "override", action = clap::ArgAction::Append)]
    pub overrides: Vec<String>,
    /// Capture or validate a snapshot with header/type hash and sampled value summaries (writes if missing)
    #[arg(long = "snapshot")]
    pub snapshot: Option<PathBuf>,
}

#[derive(Debug, Args, Clone)]
pub struct SchemaInferArgs {
    #[command(flatten)]
    pub probe: SchemaProbeArgs,
    /// Destination .schema file path (alias --schema retained for compatibility)
    #[arg(short = 'o', long = "output", alias = "schema", short_alias = 'm')]
    pub output: Option<PathBuf>,
    /// Inject empty replace arrays into the generated schema as a template when inferring
    #[arg(long = "replace-template")]
    pub replace_template: bool,
}

#[derive(Debug, Args, Clone)]
pub struct SchemaVerifyArgs {
    /// Schema file describing the expected structure
    #[arg(short = 'm', long = "schema", alias = "meta")]
    pub schema: PathBuf,
    /// One or more CSV files to verify
    #[arg(short = 'i', long = "input", required = true, action = clap::ArgAction::Append)]
    pub inputs: Vec<PathBuf>,
    /// CSV delimiter character
    #[arg(long, value_parser = parse_delimiter)]
    pub delimiter: Option<u8>,
    /// Character encoding for input files (defaults to utf-8)
    #[arg(long = "input-encoding")]
    pub input_encoding: Option<String>,
    /// Report invalid rows by summary (default) or detail. Append ':detail' and/or ':summary' and optionally a LIMIT value.
    #[arg(long = "report-invalid", value_name = "OPTIONS", num_args = 0..=3)]
    pub report_invalid: Option<Vec<String>>,
}

#[derive(Debug, Args)]
pub struct IndexArgs {
    /// Input CSV file to index
    #[arg(short, long)]
    pub input: PathBuf,
    /// Output index file (.idx)
    #[arg(short = 'o', long = "index")]
    pub index: PathBuf,
    /// Columns to include in a single ascending index (deprecated when --spec is used)
    #[arg(short = 'C', long = "columns", value_delimiter = ',')]
    pub columns: Vec<String>,
    /// Repeatable index specifications such as `col_a:asc,col_b:desc` or `fast=col_a:asc`
    #[arg(long = "spec", action = clap::ArgAction::Append)]
    pub specs: Vec<String>,
    /// Generate index variants by expanding column prefixes and direction combinations (use `|` to separate directions)
    #[arg(long = "combo", action = clap::ArgAction::Append)]
    pub combos: Vec<String>,
    /// Optional schema file describing column types
    #[arg(short = 'm', long = "schema", alias = "meta")]
    pub schema: Option<PathBuf>,
    /// Limit number of rows to scan (useful for prototyping)
    #[arg(long)]
    pub limit: Option<usize>,
    /// CSV delimiter character (supports ',', 'tab', ';', '|')
    #[arg(long, value_parser = parse_delimiter)]
    pub delimiter: Option<u8>,
    /// Character encoding of the input file (defaults to utf-8)
    #[arg(long = "input-encoding")]
    pub input_encoding: Option<String>,
}

#[derive(Debug, Args)]
pub struct ProcessArgs {
    /// Input CSV file to process
    #[arg(short = 'i', long = "input")]
    pub input: PathBuf,
    /// Output CSV file (stdout if omitted)
    #[arg(short = 'o', long = "output")]
    pub output: Option<PathBuf>,
    /// Schema file to drive typed operations and apply value replacements
    #[arg(short = 'm', long = "schema", alias = "meta")]
    pub schema: Option<PathBuf>,
    /// Existing index file to speed up operations
    #[arg(short = 'x', long = "index")]
    pub index: Option<PathBuf>,
    /// Specific index variant name to use from the selected index file
    #[arg(long = "index-variant")]
    pub index_variant: Option<String>,
    /// Sort directives of the form `column[:asc|desc]`
    #[arg(long = "sort", action = clap::ArgAction::Append)]
    pub sort: Vec<String>,
    /// Restrict output to this comma-separated list of columns
    #[arg(short = 'C', long = "columns", action = clap::ArgAction::Append)]
    pub columns: Vec<String>,
    /// Exclude this comma-separated list of columns from output
    #[arg(long = "exclude-columns", action = clap::ArgAction::Append)]
    pub exclude_columns: Vec<String>,
    /// Additional derived columns using `name=expression`
    #[arg(long = "derive", action = clap::ArgAction::Append)]
    pub derives: Vec<String>,
    /// Row-level filters such as `amount>=100` or `status = shipped`
    #[arg(long = "filter", action = clap::ArgAction::Append)]
    pub filters: Vec<String>,
    /// Evalexpr-based filter expressions that must evaluate to truthy values
    #[arg(long = "filter-expr", action = clap::ArgAction::Append)]
    pub filter_exprs: Vec<String>,
    /// Emit 1-based row numbers as the first column
    #[arg(long = "row-numbers")]
    pub row_numbers: bool,
    /// Limit number of rows emitted
    #[arg(long)]
    pub limit: Option<usize>,
    /// CSV delimiter character for reading input
    #[arg(long, value_parser = parse_delimiter)]
    pub delimiter: Option<u8>,
    /// Delimiter to use for output (defaults to input delimiter)
    #[arg(long = "output-delimiter", value_parser = parse_delimiter)]
    pub output_delimiter: Option<u8>,
    /// Character encoding of the input file (defaults to utf-8)
    #[arg(long = "input-encoding")]
    pub input_encoding: Option<String>,
    /// Character encoding for the output file/stdout (defaults to utf-8)
    #[arg(long = "output-encoding")]
    pub output_encoding: Option<String>,
    /// Normalize boolean columns in output
    #[arg(long = "boolean-format", default_value = "original")]
    pub boolean_format: BooleanFormat,
    /// Render results as a preview table on stdout (disables --output and defaults the row limit)
    #[arg(long = "preview")]
    pub preview: bool,
    /// Render output as an elastic table to stdout
    #[arg(long = "table")]
    pub table: bool,
}

#[derive(Debug, Clone, Copy, ValueEnum, PartialEq, Eq, Default)]
#[value(rename_all = "kebab-case")]
pub enum BooleanFormat {
    #[default]
    Original,
    TrueFalse,
    OneZero,
}

#[derive(Debug, Args)]
pub struct AppendArgs {
    /// One or more CSV files to append
    #[arg(short = 'i', long = "input", required = true, action = clap::ArgAction::Append)]
    pub inputs: Vec<PathBuf>,
    /// Destination CSV file (stdout if omitted)
    #[arg(short = 'o', long = "output")]
    pub output: Option<PathBuf>,
    /// Schema file to verify against
    #[arg(short = 'm', long = "schema", alias = "meta")]
    pub schema: Option<PathBuf>,
    /// CSV delimiter character
    #[arg(long, value_parser = parse_delimiter)]
    pub delimiter: Option<u8>,
    /// Character encoding for input files (defaults to utf-8)
    #[arg(long = "input-encoding")]
    pub input_encoding: Option<String>,
    /// Character encoding for the output file/stdout (defaults to utf-8)
    #[arg(long = "output-encoding")]
    pub output_encoding: Option<String>,
}

#[derive(Debug, Args)]
pub struct StatsArgs {
    /// Input CSV file to profile
    #[arg(short = 'i', long = "input")]
    pub input: PathBuf,
    /// Schema file to drive typed operations
    #[arg(short = 'm', long = "schema", alias = "meta")]
    pub schema: Option<PathBuf>,
    /// Columns to include (defaults to numeric columns)
    #[arg(short = 'C', long = "columns", action = clap::ArgAction::Append)]
    pub columns: Vec<String>,
    /// Row-level filters such as `amount>=100` or `status = shipped`
    #[arg(long = "filter", action = clap::ArgAction::Append)]
    pub filters: Vec<String>,
    /// Evalexpr-based filter expressions that must evaluate to truthy values
    #[arg(long = "filter-expr", action = clap::ArgAction::Append)]
    pub filter_exprs: Vec<String>,
    /// CSV delimiter character
    #[arg(long, value_parser = parse_delimiter)]
    pub delimiter: Option<u8>,
    /// Character encoding for input file (defaults to utf-8)
    #[arg(long = "input-encoding")]
    pub input_encoding: Option<String>,
    /// Maximum rows to scan (0 = all)
    #[arg(long, default_value_t = 0)]
    pub limit: usize,
    /// Emit distinct value counts instead of summary statistics
    #[arg(long)]
    pub frequency: bool,
    /// Maximum distinct values to display per column when --frequency is used (0 = all)
    #[arg(long, default_value_t = 0)]
    pub top: usize,
}

#[derive(Debug, Args)]
pub struct SchemaColumnsArgs {
    /// Schema file describing the columns to list
    #[arg(short = 'm', long = "schema", alias = "meta")]
    pub schema: PathBuf,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
#[value(rename_all = "kebab-case")]
pub enum JoinKind {
    Inner,
    Left,
    Right,
    Full,
}

#[derive(Debug, Args)]
pub struct JoinArgs {
    /// Left CSV input
    #[arg(long = "left")]
    pub left: PathBuf,
    /// Right CSV input
    #[arg(long = "right")]
    pub right: PathBuf,
    /// Output CSV file (stdout if omitted)
    #[arg(short = 'o', long = "output")]
    pub output: Option<PathBuf>,
    /// Comma-separated key columns from the left file
    #[arg(long = "left-key")]
    pub left_key: String,
    /// Comma-separated key columns from the right file
    #[arg(long = "right-key")]
    pub right_key: String,
    /// Join type (inner, left, right, full)
    #[arg(long = "type", value_enum, default_value = "inner")]
    pub kind: JoinKind,
    /// Schema for the left file
    #[arg(long = "left-schema", alias = "left-meta")]
    pub left_schema: Option<PathBuf>,
    /// Schema for the right file
    #[arg(long = "right-schema", alias = "right-meta")]
    pub right_schema: Option<PathBuf>,
    /// CSV delimiter character for inputs
    #[arg(long = "delimiter", value_parser = parse_delimiter)]
    pub delimiter: Option<u8>,
    /// Character encoding for the left input file (defaults to utf-8)
    #[arg(long = "left-encoding")]
    pub left_encoding: Option<String>,
    /// Character encoding for the right input file (defaults to utf-8)
    #[arg(long = "right-encoding")]
    pub right_encoding: Option<String>,
    /// Character encoding for the output file/stdout (defaults to utf-8)
    #[arg(long = "output-encoding")]
    pub output_encoding: Option<String>,
}

#[derive(Debug, Args)]
pub struct InstallArgs {
    /// Install a specific published version
    #[arg(long)]
    pub version: Option<String>,
    /// Force reinstallation even if already installed
    #[arg(long)]
    pub force: bool,
    /// Use --locked to honour Cargo.lock for dependencies
    #[arg(long)]
    pub locked: bool,
    /// Install into an alternate root directory
    #[arg(long)]
    pub root: Option<PathBuf>,
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
