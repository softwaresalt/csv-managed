pub mod append;
pub mod cli;
pub mod data;
pub mod derive;
pub mod filter;
pub mod frequency;
pub mod index;
pub mod install;
pub mod io_utils;
pub mod join;
pub mod preview;
pub mod process;
pub mod schema;
pub mod schema_cmd;
pub mod stats;
pub mod table;
pub mod verify;

use std::{env, sync::OnceLock};

use anyhow::{Context, Result};
use clap::Parser;
use log::{LevelFilter, debug, info};

use crate::cli::{Cli, Commands};

static LOGGER: OnceLock<()> = OnceLock::new();

fn init_logging() {
    LOGGER.get_or_init(|| {
        let mut builder = env_logger::Builder::from_env(env_logger::Env::default());
        if env::var("RUST_LOG").is_err() {
            builder.filter_module("csv_managed", LevelFilter::Info);
        }
        let _ = builder.format_timestamp_millis().try_init();
    });
}

pub fn run() -> Result<()> {
    init_logging();
    let cli = Cli::parse();
    match cli.command {
        Commands::Probe(args) => handle_probe(&args),
        Commands::Index(args) => handle_index(&args),
        Commands::Schema(args) => schema_cmd::execute(&args),
        Commands::Process(args) => process::execute(&args),
        Commands::Append(args) => append::execute(&args),
        Commands::Verify(args) => verify::execute(&args),
        Commands::Preview(args) => preview::execute(&args),
        Commands::Stats(args) => stats::execute(&args),
        Commands::Frequency(args) => frequency::execute(&args),
        Commands::Join(args) => join::execute(&args),
        Commands::Install(args) => install::execute(&args),
    }
}

fn handle_probe(args: &cli::ProbeArgs) -> Result<()> {
    let delimiter = io_utils::resolve_input_delimiter(&args.input, args.delimiter);
    let encoding = io_utils::resolve_encoding(args.input_encoding.as_deref())?;
    info!(
        "Probing '{}' with delimiter '{}'",
        args.input.display(),
        printable_delimiter(delimiter)
    );
    let schema = schema::infer_schema(&args.input, args.sample_rows, delimiter, encoding)
        .with_context(|| format!("Inferring schema from {input:?}", input = args.input))?;
    schema
        .save(&args.schema)
        .with_context(|| format!("Writing schema to {:?}", args.schema))?;
    info!(
        "Inferred schema for {} column(s) written to {:?}",
        schema.columns.len(),
        args.schema
    );
    Ok(())
}

fn handle_index(args: &cli::IndexArgs) -> Result<()> {
    let delimiter = io_utils::resolve_input_delimiter(&args.input, args.delimiter);
    let encoding = io_utils::resolve_encoding(args.input_encoding.as_deref())?;
    info!(
        "Building index for '{}' using delimiter '{}'",
        args.input.display(),
        printable_delimiter(delimiter)
    );
    let schema = match &args.schema {
        Some(path) => Some(
            schema::Schema::load(path).with_context(|| format!("Loading schema from {path:?}"))?,
        ),
        None => None,
    };
    let mut definitions = Vec::new();
    for spec in &args.specs {
        let definition = index::IndexDefinition::parse(spec)
            .with_context(|| format!("Parsing index specification '{spec}'"))?;
        definitions.push(definition);
    }
    for combo in &args.combos {
        let expanded = index::IndexDefinition::expand_combo_spec(combo)
            .with_context(|| format!("Parsing index combination '{combo}'"))?;
        definitions.extend(expanded);
    }
    if definitions.is_empty() {
        let columns = args
            .columns
            .iter()
            .map(|c| c.trim())
            .filter(|c| !c.is_empty())
            .map(|c| c.to_string())
            .collect::<Vec<_>>();
        let definition = index::IndexDefinition::from_columns(columns)
            .context("Parsing --columns for index build")?;
        definitions.push(definition);
    }
    debug!("Index definitions: {:?}", definitions.len());
    let index = index::CsvIndex::build(
        &args.input,
        &definitions,
        schema.as_ref(),
        args.limit,
        delimiter,
        encoding,
    )
    .with_context(|| format!("Building index for {:?}", args.input))?;
    let row_count = index.row_count();
    index
        .save(&args.index)
        .with_context(|| format!("Writing index to {:?}", args.index))?;
    info!(
        "Index with {} variant(s) for {} row(s) written to {:?}",
        index.variants().len(),
        row_count,
        args.index
    );
    for variant in index.variants() {
        info!("  • {}", variant.describe());
    }
    Ok(())
}

pub(crate) fn printable_delimiter(delimiter: u8) -> String {
    match delimiter {
        b',' => ",".to_string(),
        b'\t' => "\\t".to_string(),
        b'\n' => "\\n".to_string(),
        other => (other as char).to_string(),
    }
}
