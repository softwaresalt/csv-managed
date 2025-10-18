use std::fmt;

use anyhow::{Context, Result, anyhow, bail};
use chrono::{NaiveDate, NaiveDateTime};
use evalexpr;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::schema::ColumnType;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Value {
    String(String),
    Integer(i64),
    Float(f64),
    Boolean(bool),
    Date(NaiveDate),
    DateTime(NaiveDateTime),
    Guid(Uuid),
}

impl Eq for Value {}

impl Value {
    pub fn as_display(&self) -> String {
        match self {
            Value::String(s) => s.clone(),
            Value::Integer(i) => i.to_string(),
            Value::Float(f) => {
                if f.fract() == 0.0 {
                    (*f as i64).to_string()
                } else {
                    f.to_string()
                }
            }
            Value::Boolean(b) => b.to_string(),
            Value::Date(d) => d.format("%Y-%m-%d").to_string(),
            Value::DateTime(dt) => dt.format("%Y-%m-%d %H:%M:%S").to_string(),
            Value::Guid(g) => g.to_string(),
        }
    }
}

impl Ord for Value {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match (self, other) {
            (Value::String(a), Value::String(b)) => a.cmp(b),
            (Value::Integer(a), Value::Integer(b)) => a.cmp(b),
            (Value::Float(a), Value::Float(b)) => a.total_cmp(b),
            (Value::Boolean(a), Value::Boolean(b)) => a.cmp(b),
            (Value::Date(a), Value::Date(b)) => a.cmp(b),
            (Value::DateTime(a), Value::DateTime(b)) => a.cmp(b),
            (Value::Guid(a), Value::Guid(b)) => a.cmp(b),
            _ => panic!("Cannot compare heterogeneous Value variants"),
        }
    }
}

impl PartialOrd for Value {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_display())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ComparableValue(pub Option<Value>);

impl Ord for ComparableValue {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match (&self.0, &other.0) {
            (None, None) => std::cmp::Ordering::Equal,
            (None, Some(_)) => std::cmp::Ordering::Less,
            (Some(_), None) => std::cmp::Ordering::Greater,
            (Some(left), Some(right)) => left.cmp(right),
        }
    }
}

impl PartialOrd for ComparableValue {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

pub fn parse_naive_date(value: &str) -> Result<NaiveDate> {
    const DATE_FORMATS: &[&str] = &["%Y-%m-%d", "%d/%m/%Y", "%m/%d/%Y", "%Y/%m/%d", "%d-%m-%Y"];
    for fmt in DATE_FORMATS {
        if let Ok(parsed) = NaiveDate::parse_from_str(value, fmt) {
            return Ok(parsed);
        }
    }
    Err(anyhow!("Failed to parse '{value}' as date"))
}

pub fn parse_naive_datetime(value: &str) -> Result<NaiveDateTime> {
    const DATETIME_FORMATS: &[&str] = &[
        "%Y-%m-%d %H:%M:%S",
        "%Y-%m-%dT%H:%M:%S",
        "%d/%m/%Y %H:%M:%S",
        "%m/%d/%Y %H:%M:%S",
        "%Y-%m-%d %H:%M",
        "%Y-%m-%dT%H:%M",
    ];
    for fmt in DATETIME_FORMATS {
        if let Ok(parsed) = NaiveDateTime::parse_from_str(value, fmt) {
            return Ok(parsed);
        }
    }
    Err(anyhow!("Failed to parse '{value}' as datetime"))
}

pub fn normalize_column_name(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            'a'..='z' | 'A'..='Z' | '0'..='9' => c,
            _ => '_',
        })
        .collect::<String>()
        .to_ascii_lowercase()
}

pub fn parse_typed_value(value: &str, ty: &ColumnType) -> Result<Option<Value>> {
    if value.is_empty() {
        return Ok(None);
    }
    let parsed = match ty {
        ColumnType::String => Value::String(value.to_string()),
        ColumnType::Integer => {
            let parsed: i64 = value
                .parse()
                .with_context(|| format!("Failed to parse '{value}' as integer"))?;
            Value::Integer(parsed)
        }
        ColumnType::Float => {
            let parsed: f64 = value
                .parse()
                .with_context(|| format!("Failed to parse '{value}' as float"))?;
            Value::Float(parsed)
        }
        ColumnType::Boolean => {
            let lowered = value.to_ascii_lowercase();
            let parsed = match lowered.as_str() {
                "true" | "t" | "yes" | "y" | "1" => true,
                "false" | "f" | "no" | "n" | "0" => false,
                _ => bail!("Failed to parse '{value}' as boolean"),
            };
            Value::Boolean(parsed)
        }
        ColumnType::Date => {
            let parsed = parse_naive_date(value)?;
            Value::Date(parsed)
        }
        ColumnType::DateTime => {
            let parsed = parse_naive_datetime(value)?;
            Value::DateTime(parsed)
        }
        ColumnType::Guid => {
            let trimmed = value.trim().trim_matches(|c| matches!(c, '{' | '}'));
            let parsed = Uuid::parse_str(trimmed)
                .with_context(|| format!("Failed to parse '{{value}}' as GUID"))?;
            Value::Guid(parsed)
        }
    };
    Ok(Some(parsed))
}

pub fn value_to_evalexpr(value: &Value) -> evalexpr::Value {
    match value {
        Value::String(s) => evalexpr::Value::String(s.clone()),
        Value::Integer(i) => evalexpr::Value::Int(*i),
        Value::Float(f) => evalexpr::Value::Float(*f),
        Value::Boolean(b) => evalexpr::Value::Boolean(*b),
        Value::Date(d) => evalexpr::Value::String(d.format("%Y-%m-%d").to_string()),
        Value::DateTime(dt) => evalexpr::Value::String(dt.format("%Y-%m-%d %H:%M:%S").to_string()),
        Value::Guid(g) => evalexpr::Value::String(g.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{NaiveDate, NaiveDateTime};
    use evalexpr::Value as EvalValue;
    use uuid::Uuid;

    #[test]
    fn normalize_column_name_replaces_non_alphanumeric() {
        assert_eq!(normalize_column_name("Order ID"), "order_id");
        assert_eq!(normalize_column_name("$Percent%"), "_percent_");
    }

    #[test]
    fn parse_naive_date_supports_multiple_formats() {
        let expected = NaiveDate::from_ymd_opt(2024, 5, 6).unwrap();
        assert_eq!(parse_naive_date("2024-05-06").unwrap(), expected);
        assert_eq!(parse_naive_date("06/05/2024").unwrap(), expected);
        assert_eq!(parse_naive_date("2024/05/06").unwrap(), expected);
    }

    #[test]
    fn parse_naive_datetime_supports_multiple_formats() {
        let expected =
            NaiveDateTime::parse_from_str("2024-05-06 14:30:00", "%Y-%m-%d %H:%M:%S").unwrap();
        assert_eq!(
            parse_naive_datetime("2024-05-06T14:30:00").unwrap(),
            expected
        );
        assert_eq!(
            parse_naive_datetime("06/05/2024 14:30:00").unwrap(),
            expected
        );
        assert_eq!(parse_naive_datetime("2024-05-06 14:30").unwrap(), expected);
    }

    #[test]
    fn parse_typed_value_handles_empty_and_boolean_inputs() {
        assert_eq!(parse_typed_value("", &ColumnType::Integer).unwrap(), None);

        let truthy = parse_typed_value("Yes", &ColumnType::Boolean)
            .unwrap()
            .unwrap();
        assert_eq!(truthy, Value::Boolean(true));

        let falsy = parse_typed_value("0", &ColumnType::Boolean)
            .unwrap()
            .unwrap();
        assert_eq!(falsy, Value::Boolean(false));

        assert!(parse_typed_value("maybe", &ColumnType::Boolean).is_err());
    }

    #[test]
    fn parse_typed_value_supports_guid_inputs() {
        let raw = "550e8400-e29b-41d4-a716-446655440000";
        let parsed = parse_typed_value(raw, &ColumnType::Guid).unwrap().unwrap();
        match parsed {
            Value::Guid(g) => {
                assert_eq!(g, Uuid::parse_str(raw).unwrap());
            }
            other => panic!("Expected GUID value, got {other:?}"),
        }

        let braced = "{550e8400-e29b-41d4-a716-446655440000}";
        let parsed_braced = parse_typed_value(braced, &ColumnType::Guid)
            .unwrap()
            .unwrap();
        assert!(matches!(parsed_braced, Value::Guid(_)));

        assert!(parse_typed_value("not-a-guid", &ColumnType::Guid).is_err());
    }

    #[test]
    fn value_to_evalexpr_preserves_variants() {
        assert_eq!(value_to_evalexpr(&Value::Integer(42)), EvalValue::Int(42));
        assert_eq!(
            value_to_evalexpr(&Value::Boolean(false)),
            EvalValue::Boolean(false)
        );

        let date = NaiveDate::from_ymd_opt(2024, 5, 6).unwrap();
        assert_eq!(
            value_to_evalexpr(&Value::Date(date)),
            EvalValue::String("2024-05-06".to_string())
        );
    }

    #[test]
    fn comparable_value_orders_none_before_some() {
        let none = ComparableValue(None);
        let some = ComparableValue(Some(Value::Integer(0)));
        assert!(none < some);
    }
}
