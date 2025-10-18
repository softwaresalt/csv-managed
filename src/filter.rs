use anyhow::{Result, anyhow};

use crate::{
    data::{Value, parse_typed_value},
    schema::{ColumnType, Schema},
};

#[derive(Debug, Clone, Copy)]
pub enum ComparisonOperator {
    Eq,
    NotEq,
    Gt,
    Ge,
    Lt,
    Le,
    Contains,
    StartsWith,
    EndsWith,
}

#[derive(Debug, Clone)]
pub struct FilterCondition {
    pub column: String,
    pub operator: ComparisonOperator,
    pub raw_value: String,
}

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
            .map(|c| c.data_type.clone())
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
