//! Column listing from a schema file.
//!
//! Reads a schema YAML file and renders its column names, types, and aliases
//! as an ASCII table.

use anyhow::{Context, Result};
use log::info;

use crate::{cli::SchemaColumnsArgs, schema::Schema, table};

pub fn execute(args: &SchemaColumnsArgs) -> Result<()> {
    let schema = Schema::load(&args.schema)
        .with_context(|| format!("Loading schema from {schema:?}", schema = args.schema))?;

    if schema.columns.is_empty() {
        info!("Schema {:?} does not define any columns", args.schema);
        return Ok(());
    }

    let mut rows = Vec::with_capacity(schema.columns.len());
    for (idx, column) in schema.columns.iter().enumerate() {
        let position = (idx + 1).to_string();
        let original_name = column.name.clone();
        let datatype = column.datatype.to_string();
        let output_name = column.output_name().to_string();
        let rename = if output_name != column.name {
            output_name
        } else {
            String::new()
        };
        rows.push(vec![position, original_name, datatype, rename]);
    }

    let headers = vec![
        "#".to_string(),
        "name".to_string(),
        "type".to_string(),
        "output".to_string(),
    ];
    table::print_table(&headers, &rows);
    info!(
        "Listed {} column(s) from {:?}",
        schema.columns.len(),
        args.schema
    );
    Ok(())
}
