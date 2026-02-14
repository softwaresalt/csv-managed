//! Row parsing and filter expression evaluation helpers.
//!
//! Provides [`parse_typed_row()`] which converts a raw string row into typed
//! [`Value`] cells using a schema's column definitions
//! (including value normalization via datatype mappings and replacements).
//!
//! Also provides [`evaluate_filter_expressions()`] which evaluates `--filter-expr`
//! boolean expressions against a row's context.

use anyhow::Result;

use crate::{
    data::{Value, parse_typed_value},
    expr,
    schema::Schema,
};

pub fn parse_typed_row(schema: &Schema, raw: &[String]) -> Result<Vec<Option<Value>>> {
    schema
        .columns
        .iter()
        .enumerate()
        .map(|(idx, column)| {
            let value = raw.get(idx).map(|s| s.as_str()).unwrap_or("");
            let normalized = column.normalize_value(value);
            parse_typed_value(normalized.as_ref(), &column.datatype)
        })
        .collect()
}

pub fn evaluate_filter_expressions(
    expressions: &[String],
    headers: &[String],
    raw_row: &[String],
    typed_row: &[Option<Value>],
    row_number: Option<usize>,
) -> Result<bool> {
    if expressions.is_empty() {
        return Ok(true);
    }
    let context = expr::build_context(headers, raw_row, typed_row, row_number)?;
    for expression in expressions {
        if !expr::evaluate_expression_to_bool(expression, &context)? {
            return Ok(false);
        }
    }
    Ok(true)
}
