pub mod append;
pub mod cli;
pub mod columns;
pub mod data;
pub mod derive;
pub mod filter;
pub mod fix;
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
        Commands::Fix(args) => fix::execute(&args),
        Commands::Append(args) => append::execute(&args),
        Commands::Verify(args) => verify::execute(&args),
        Commands::Preview(args) => preview::execute(&args),
        Commands::Stats(args) => stats::execute(&args),
        Commands::Frequency(args) => frequency::execute(&args),
        Commands::Join(args) => join::execute(&args),
        Commands::Install(args) => install::execute(&args),
        Commands::Columns(args) => columns::execute(&args),
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
    let mut schema = schema::infer_schema(&args.input, args.sample_rows, delimiter, encoding)
        .with_context(|| format!("Inferring schema from {input:?}", input = args.input))?;
    if args.mapping {
        apply_default_name_mappings(&mut schema);
    }
    if args.replace_template {
        schema
            .save_with_replace_template(&args.schema)
            .with_context(|| format!("Writing schema to {:?}", args.schema))?;
    } else {
        schema
            .save(&args.schema)
            .with_context(|| format!("Writing schema to {:?}", args.schema))?;
    }
    info!(
        "Inferred schema for {} column(s) written to {:?}",
        schema.columns.len(),
        args.schema
    );

    if args.mapping {
        emit_mappings(&schema);
    }
    Ok(())
}

fn apply_default_name_mappings(schema: &mut schema::Schema) {
    for column in &mut schema.columns {
        if column.rename.is_none() {
            column.rename = Some(to_lower_snake_case(&column.name));
        }
    }
}

fn to_lower_snake_case(value: &str) -> String {
    let mut result = String::new();
    let mut chars = value.chars().peekable();
    let mut last_was_separator = true;
    let mut last_was_upper = false;
    while let Some(ch) = chars.next() {
        if ch.is_ascii_alphanumeric() {
            if ch.is_ascii_uppercase() {
                let next_is_lowercase = chars
                    .peek()
                    .map(|c| c.is_ascii_lowercase())
                    .unwrap_or(false);
                if !result.is_empty()
                    && (!last_was_separator && (!last_was_upper || next_is_lowercase))
                    && !result.ends_with('_')
                {
                    result.push('_');
                }
                result.push(ch.to_ascii_lowercase());
                last_was_separator = false;
                last_was_upper = true;
            } else {
                if !result.is_empty() && last_was_separator && !result.ends_with('_') {
                    result.push('_');
                }
                result.push(ch.to_ascii_lowercase());
                last_was_separator = false;
                last_was_upper = false;
            }
        } else {
            if !result.ends_with('_') && !result.is_empty() {
                result.push('_');
            }
            last_was_separator = true;
            last_was_upper = false;
        }
    }
    while result.ends_with('_') {
        result.pop();
    }
    if result.is_empty() {
        value.to_ascii_lowercase()
    } else {
        result
    }
}

fn emit_mappings(schema: &schema::Schema) {
    if schema.columns.is_empty() {
        println!("No columns found to emit mappings.");
        return;
    }
    let mut rows = Vec::with_capacity(schema.columns.len());
    for (idx, column) in schema.columns.iter().enumerate() {
        let mapping = format!("{}:{}->", column.name, column.datatype.as_str());
        rows.push(vec![
            (idx + 1).to_string(),
            column.name.clone(),
            column.datatype.to_string(),
            mapping,
        ]);
    }
    let headers = vec![
        "#".to_string(),
        "name".to_string(),
        "type".to_string(),
        "mapping".to_string(),
    ];
    table::print_table(&headers, &rows);
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

#[cfg(test)]
mod tests {
    use super::to_lower_snake_case;

    #[test]
    fn converts_camel_case_to_snake() {
        assert_eq!(to_lower_snake_case("OrderDate"), "order_date");
    }

    #[test]
    fn collapses_separators() {
        assert_eq!(to_lower_snake_case("customer-name"), "customer_name");
        assert_eq!(to_lower_snake_case("customer  name"), "customer_name");
    }

    #[test]
    fn handles_acronyms() {
        assert_eq!(to_lower_snake_case("APIKey"), "api_key");
        assert_eq!(to_lower_snake_case("HTTPStatus"), "http_status");
    }
}
