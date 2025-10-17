pub mod cli;
pub mod data;
pub mod derive;
pub mod filter;
pub mod index;
pub mod metadata;
pub mod process;

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
        Commands::Process(args) => process::execute(&args),
    }
}

fn handle_probe(args: &cli::ProbeArgs) -> Result<()> {
    info!(
        "Probing '{}' with delimiter '{}'",
        args.input.display(),
        printable_delimiter(args.delimiter)
    );
    let schema = metadata::infer_schema(&args.input, args.sample_rows, args.delimiter)
        .with_context(|| format!("Inferring schema from {:?}", args.input))?;
    schema
        .save(&args.meta)
        .with_context(|| format!("Writing metadata to {:?}", args.meta))?;
    info!(
        "Inferred schema for {} column(s) written to {:?}",
        schema.columns.len(),
        args.meta
    );
    Ok(())
}

fn handle_index(args: &cli::IndexArgs) -> Result<()> {
    info!(
        "Building index for '{}' using delimiter '{}'",
        args.input.display(),
        printable_delimiter(args.delimiter)
    );
    let schema = match &args.meta {
        Some(path) => Some(
            metadata::Schema::load(path)
                .with_context(|| format!("Loading metadata from {path:?}"))?,
        ),
        None => None,
    };
    let columns = args
        .columns
        .iter()
        .map(|c| c.trim())
        .filter(|c| !c.is_empty())
        .map(|c| c.to_string())
        .collect::<Vec<_>>();
    debug!("Index columns: {:?}", columns);
    let index = index::CsvIndex::build(
        &args.input,
        &columns,
        schema.as_ref(),
        args.limit,
        args.delimiter,
    )
    .with_context(|| format!("Building index for {:?}", args.input))?;
    let row_count = index.row_count();
    index
        .save(&args.index)
        .with_context(|| format!("Writing index to {:?}", args.index))?;
    info!(
        "Index for {} row(s) across {} column(s) written to {:?}",
        row_count,
        columns.len(),
        args.index
    );
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
