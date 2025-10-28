pub mod append;
pub mod cli;
pub mod columns;
pub mod data;
pub mod derive;
pub mod expr;
pub mod filter;
pub mod frequency;
pub mod index;
pub mod install;
pub mod io_utils;
pub mod join;
pub mod process;
pub mod rows;
pub mod schema;
pub mod schema_cmd;
pub mod stats;
pub mod table;
pub mod verify;

use std::{env, ffi::OsString, sync::OnceLock, time::Instant};

use anyhow::{Context, Result};
use chrono::{SecondsFormat, Utc};
use clap::Parser;
use log::{LevelFilter, debug, error, info};

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
    let cli = Cli::parse_from(preprocess_cli_args(env::args_os()));
    match cli.command {
        Commands::Index(args) => run_operation("index", || handle_index(&args)),
        Commands::Schema(args) => run_operation("schema", || schema_cmd::execute(&args)),
        Commands::Process(args) => run_operation("process", || process::execute(&args)),
        Commands::Append(args) => run_operation("append", || append::execute(&args)),
        Commands::Stats(args) => run_operation("stats", || stats::execute(&args)),
        // Commands::Join(args) => run_operation("join", || join::execute(&args)),
        Commands::Install(args) => run_operation("install", || install::execute(&args)),
    }
}

fn preprocess_cli_args<I>(args: I) -> Vec<OsString>
where
    I: IntoIterator<Item = OsString>,
{
    let mut processed = Vec::new();
    for arg in args {
        if let Some(value) = arg.to_str()
            && let Some(rest) = value.strip_prefix("--report-invalid:")
        {
            processed.push(OsString::from("--report-invalid"));
            for segment in rest.split(':').filter(|segment| !segment.is_empty()) {
                processed.push(OsString::from(segment));
            }
            continue;
        }
        processed.push(arg);
    }
    processed
}

fn run_operation<F>(name: &str, op: F) -> Result<()>
where
    F: FnOnce() -> Result<()>,
{
    let start_clock = Utc::now();
    let start_instant = Instant::now();
    let result = op();
    let end_clock = Utc::now();
    let duration_secs = start_instant.elapsed().as_secs_f64();
    let start_str = start_clock.to_rfc3339_opts(SecondsFormat::Millis, true);
    let end_str = end_clock.to_rfc3339_opts(SecondsFormat::Millis, true);

    match &result {
        Ok(_) => info!(
            "Operation '{name}' completed (status=ok)\nstart: {start_str}\nend: {end_str}\nduration_secs: {duration_secs:.3}"
        ),
        Err(err) => error!(
            "Operation '{name}' failed (status=error)\nstart: {start_str}\nend: {end_str}\nduration_secs: {duration_secs:.3}\nerror: {err:?}"
        ),
    }

    result
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
    for covering in &args.coverings {
        let expanded = index::IndexDefinition::expand_covering_spec(covering)
            .with_context(|| format!("Parsing index covering '{covering}'"))?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn to_strings(args: &[OsString]) -> Vec<String> {
        args.iter()
            .map(|value| value.to_string_lossy().to_string())
            .collect()
    }

    #[test]
    fn preprocess_cli_args_expands_report_invalid_segments() {
        let processed = preprocess_cli_args(vec![
            OsString::from("csv-managed"),
            OsString::from("--report-invalid:stats:counts"),
            OsString::from("--dry-run"),
        ]);
        let tokens = to_strings(&processed);
        assert_eq!(
            tokens,
            vec![
                "csv-managed",
                "--report-invalid",
                "stats",
                "counts",
                "--dry-run",
            ]
        );
    }

    proptest! {
        #[test]
        fn preprocess_cli_args_splits_report_invalid_segments_prop(
            segments in proptest::collection::vec("[A-Za-z0-9_-]{1,8}", 1..5)
        ) {
            let mut arg = String::from("--report-invalid");
            for segment in &segments {
                arg.push(':');
                arg.push_str(segment);
            }
            let processed = preprocess_cli_args(vec![
                OsString::from("csv-managed"),
                OsString::from(arg),
            ]);
            let tokens = to_strings(&processed);
            prop_assert_eq!(tokens[0].as_str(), "csv-managed");
            prop_assert_eq!(tokens[1].as_str(), "--report-invalid");
            for (idx, segment) in segments.iter().enumerate() {
                prop_assert_eq!(tokens[idx + 2].as_str(), segment.as_str());
            }
        }
    }
}
