//! Row-level filter parsing and evaluation.
//!
//! Translates `--filter` CLI strings into typed [`FilterCondition`] values and
//! evaluates them against each row during streaming. Supports equality, ordering,
//! and string-matching operators. Multiple conditions are combined with AND
//! semantics.
//!
//! # Complexity
//!
//! Parsing is O(f) where f is the number of filter strings. Evaluation is O(f)
//! per row, with typed comparison delegated to [`crate::data::parse_typed_value`].

use anyhow::{Result, anyhow};

use crate::{
    data::{Value, parse_typed_value},
    schema::{ColumnType, Schema},
};

/// Comparison operators supported in `--filter` expressions (equality, ordering, string matching).
#[derive(Debug, Clone, Copy)]
pub enum ComparisonOperator {
    /// Exact equality (`=`).
    Eq,
    /// Inequality (`!=`).
    NotEq,
    /// Greater than (`>`).
    Gt,
    /// Greater than or equal (`>=`).
    Ge,
    /// Less than (`<`).
    Lt,
    /// Less than or equal (`<=`).
    Le,
    /// Case-sensitive substring match.
    Contains,
    /// Case-sensitive prefix match.
    StartsWith,
    /// Case-sensitive suffix match.
    EndsWith,
}

/// A parsed filter clause binding a column name, comparison operator, and raw right-hand-side value.
#[derive(Debug, Clone)]
pub struct FilterCondition {
    pub column: String,
    pub operator: ComparisonOperator,
    pub raw_value: String,
}

/// Parses a slice of raw `--filter` strings into typed [`FilterCondition`] values.
pub fn parse_filters(filters: &[String]) -> Result<Vec<FilterCondition>> {
    filters.iter().map(|f| parse_filter(f)).collect()
}

fn parse_filter(filter: &str) -> Result<FilterCondition> {
    let trimmed = filter.trim();
    if trimmed.is_empty() {
        return Err(anyhow!("Empty filter expression"));
    }

    let lowered = trimmed.to_ascii_lowercase();
    for (needle, op) in [
        (" contains ", ComparisonOperator::Contains),
        (" startswith ", ComparisonOperator::StartsWith),
        (" endswith ", ComparisonOperator::EndsWith),
    ] {
        if let Some(idx) = lowered.find(needle) {
            let (left, right_with_space) = trimmed.split_at(idx);
            let right = right_with_space[needle.len()..].trim();
            return Ok(FilterCondition {
                column: left.trim().to_string(),
                operator: op,
                raw_value: unquote(right)?.to_string(),
            });
        }
    }

    for needle in ["!=", ">=", "<=", "=", ">", "<"] {
        if let Some(idx) = trimmed.find(needle) {
            let op = match needle {
                "=" => ComparisonOperator::Eq,
                "!=" => ComparisonOperator::NotEq,
                ">" => ComparisonOperator::Gt,
                ">=" => ComparisonOperator::Ge,
                "<" => ComparisonOperator::Lt,
                "<=" => ComparisonOperator::Le,
                _ => unreachable!(),
            };
            let left = trimmed[..idx].trim();
            let right = trimmed[idx + needle.len()..].trim();
            return Ok(FilterCondition {
                column: left.to_string(),
                operator: op,
                raw_value: unquote(right)?.to_string(),
            });
        }
    }

    Err(anyhow!("Failed to parse filter expression '{trimmed}'"))
}

fn unquote(value: &str) -> Result<&str> {
    if value.len() >= 2 {
        let bytes = value.as_bytes();
        if (bytes[0] == b'"' && bytes[value.len() - 1] == b'"')
            || (bytes[0] == b'\'' && bytes[value.len() - 1] == b'\'')
        {
            return Ok(&value[1..value.len() - 1]);
        }
    }
    Ok(value)
}

/// Evaluates all filter conditions against a single row, returning `true` only when every
/// condition passes (AND semantics).
pub fn evaluate_conditions(
    conditions: &[FilterCondition],
    schema: &Schema,
    headers: &[String],
    raw_row: &[String],
    typed_row: &[Option<Value>],
) -> Result<bool> {
    for condition in conditions {
        let Some(col_index) = schema.column_index(&condition.column).or_else(|| {
            headers
                .iter()
                .position(|header| header == &condition.column)
        }) else {
            return Err(anyhow!(
                "Column '{}' not found for filter",
                condition.column
            ));
        };
        let column_type = schema
            .columns
            .get(col_index)
            .map(|c| c.datatype.clone())
            .unwrap_or(ColumnType::String);
        if !evaluate_condition(
            condition,
            &column_type,
            raw_row.get(col_index).map(|s| s.as_str()),
            typed_row.get(col_index).and_then(|v| v.as_ref()),
        )? {
            return Ok(false);
        }
    }
    Ok(true)
}

fn evaluate_condition(
    condition: &FilterCondition,
    column_type: &ColumnType,
    raw_value: Option<&str>,
    typed_value: Option<&Value>,
) -> Result<bool> {
    let candidate_typed = match typed_value {
        Some(value) => Some(value.clone()),
        None => {
            if let Some(raw) = raw_value {
                parse_typed_value(raw, column_type)?
            } else {
                None
            }
        }
    };

    use ComparisonOperator::*;
    match condition.operator {
        Contains | StartsWith | EndsWith => {
            let raw = raw_value.unwrap_or("");
            let needle = condition.raw_value.as_str();
            let cmp = match condition.operator {
                Contains => raw.contains(needle),
                StartsWith => raw.starts_with(needle),
                EndsWith => raw.ends_with(needle),
                _ => unreachable!(),
            };
            Ok(cmp)
        }
        Eq | NotEq | Gt | Ge | Lt | Le => {
            let rhs_value = parse_typed_value(&condition.raw_value, column_type)?;
            match (candidate_typed, rhs_value) {
                (Some(left), Some(right)) => match condition.operator {
                    Eq => Ok(left == right),
                    NotEq => Ok(left != right),
                    Gt => Ok(left > right),
                    Ge => Ok(left >= right),
                    Lt => Ok(left < right),
                    Le => Ok(left <= right),
                    _ => unreachable!(),
                },
                (None, None) => Ok(matches!(condition.operator, Eq | Ge | Le)),
                (None, Some(_)) => Ok(matches!(condition.operator, NotEq)),
                (Some(_), None) => Ok(matches!(condition.operator, NotEq)),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_filters_rejects_empty_filter_string() {
        let filters = vec!["".to_string()];
        let err = parse_filters(&filters).expect_err("empty filter should fail");
        assert!(
            err.to_string().contains("Empty filter"),
            "Expected 'Empty filter' error, got: {err}"
        );
    }

    #[test]
    fn parse_filters_rejects_missing_operator() {
        let filters = vec!["column_without_operator".to_string()];
        let err = parse_filters(&filters).expect_err("missing operator should fail");
        assert!(
            err.to_string().contains("parse filter"),
            "Expected parse error, got: {err}"
        );
    }

    #[test]
    fn parse_filters_accepts_valid_operators() {
        let cases = vec![
            "col = value",
            "col != value",
            "col > 10",
            "col >= 10",
            "col < 10",
            "col <= 10",
            "col contains needle",
            "col startswith pre",
            "col endswith suf",
        ];
        for case in cases {
            let result = parse_filters(&[case.to_string()]);
            assert!(
                result.is_ok(),
                "Expected success for '{case}', got: {result:?}"
            );
        }
    }
}
