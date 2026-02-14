//! Value types, typed parsing, and type-system primitives for CSV cell data.
//!
//! This module defines the [`Value`] enum (mirroring [`crate::schema::ColumnType`]),
//! typed parsing functions for booleans, dates, datetimes, times, GUIDs, currencies,
//! and fixed-precision decimals. It also provides currency/decimal rounding strategies,
//! evalexpr conversion helpers, and column-name normalization.
//!
//! ## Complexity
//!
//! All parsing functions operate in O(n) time over the input string length.
//! Currency and decimal parsers perform a single sanitization pass before
//! delegating to `rust_decimal::Decimal::from_str`.

use std::fmt;

use anyhow::{Context, Result, anyhow, bail, ensure};
use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use evalexpr;
use rust_decimal::Decimal;
use rust_decimal::RoundingStrategy;
use rust_decimal::prelude::ToPrimitive;
use serde::{Deserialize, Deserializer, Serialize, Serializer, de, ser::SerializeStruct};
use std::str::FromStr;
use uuid::Uuid;

use crate::schema::{ColumnType, DecimalSpec};

pub const CURRENCY_ALLOWED_SCALES: [u32; 2] = [2, 4];

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct FixedDecimalValue {
    amount: Decimal,
    precision: u32,
    scale: u32,
}

impl FixedDecimalValue {
    pub fn parse(raw: &str, spec: &DecimalSpec) -> Result<Self> {
        let decimal = parse_decimal_literal(raw)?;
        Self::from_decimal(decimal, spec, None)
    }

    pub fn from_decimal(
        value: Decimal,
        spec: &DecimalSpec,
        strategy: Option<&str>,
    ) -> Result<Self> {
        let mut decimal = value;
        if let Some(strategy) = strategy {
            decimal = match strategy {
                "truncate" => decimal.round_dp_with_strategy(spec.scale, RoundingStrategy::ToZero),
                "round" | "round-half-up" | "roundhalfup" => decimal
                    .round_dp_with_strategy(spec.scale, RoundingStrategy::MidpointAwayFromZero),
                other => bail!("Unsupported decimal rounding strategy '{other}'"),
            };
        }
        Self::validate_decimal(&decimal, spec)?;
        let mut quantized = decimal;
        if quantized.scale() < spec.scale {
            quantized.rescale(spec.scale);
        }
        Ok(Self {
            amount: quantized,
            precision: spec.precision,
            scale: spec.scale,
        })
    }

    pub fn amount(&self) -> &Decimal {
        &self.amount
    }

    pub fn precision(&self) -> u32 {
        self.precision
    }

    pub fn scale(&self) -> u32 {
        self.scale
    }

    pub fn to_string_fixed(&self) -> String {
        format_decimal_with_scale(self.amount, self.scale as usize)
    }

    pub fn to_f64(&self) -> Option<f64> {
        self.amount.to_f64()
    }

    fn validate_decimal(decimal: &Decimal, spec: &DecimalSpec) -> Result<()> {
        ensure!(
            decimal.scale() <= spec.scale,
            "Decimal values must not exceed scale {} (found {})",
            spec.scale,
            decimal.scale()
        );
        let integer_digits = count_integer_digits(decimal);
        let max_integer_digits = (spec.precision - spec.scale) as usize;
        ensure!(
            integer_digits <= max_integer_digits,
            "Decimal values must not exceed {} digit(s) to the left of the decimal point",
            max_integer_digits
        );
        Ok(())
    }
}

impl Serialize for FixedDecimalValue {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("FixedDecimalValue", 3)?;
        state.serialize_field("amount", &self.to_string_fixed())?;
        state.serialize_field("precision", &self.precision)?;
        state.serialize_field("scale", &self.scale)?;
        state.end()
    }
}

impl<'de> Deserialize<'de> for FixedDecimalValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct FixedDecimalValueRepr {
            amount: String,
            precision: u32,
            scale: u32,
        }

        let repr = FixedDecimalValueRepr::deserialize(deserializer)?;
        let spec = DecimalSpec::new(repr.precision, repr.scale)
            .map_err(|err| de::Error::custom(err.to_string()))?;
        let decimal =
            Decimal::from_str(&repr.amount).map_err(|err| de::Error::custom(err.to_string()))?;
        FixedDecimalValue::from_decimal(decimal, &spec, None)
            .map_err(|err| de::Error::custom(err.to_string()))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct CurrencyValue {
    amount: Decimal,
}

impl CurrencyValue {
    pub fn parse(raw: &str) -> Result<Self> {
        let decimal = parse_currency_decimal(raw)?;
        Self::from_decimal(decimal).with_context(|| format!("Parsing '{raw}' as currency"))
    }

    pub fn from_decimal(mut amount: Decimal) -> Result<Self> {
        match amount.scale() {
            0 => {
                amount.rescale(2);
            }
            scale if CURRENCY_ALLOWED_SCALES.contains(&scale) => {}
            other => {
                bail!("Currency values must have 2 or 4 decimal places (found {other})");
            }
        }
        Ok(Self { amount })
    }

    pub fn quantize(mut amount: Decimal, scale: u32, strategy: Option<&str>) -> Result<Self> {
        ensure!(
            CURRENCY_ALLOWED_SCALES.contains(&scale),
            "Currency scale must be 2 or 4"
        );
        match strategy {
            Some("truncate") => {
                amount = amount.round_dp_with_strategy(scale, RoundingStrategy::ToZero);
            }
            Some("round") | Some("round-half-up") | Some("roundhalfup") | None => {
                amount =
                    amount.round_dp_with_strategy(scale, RoundingStrategy::MidpointAwayFromZero);
            }
            Some(other) => {
                bail!("Unsupported currency rounding strategy '{other}'");
            }
        }
        Self::from_decimal(amount)
    }

    pub fn amount(&self) -> &Decimal {
        &self.amount
    }

    pub fn scale(&self) -> u32 {
        self.amount.scale()
    }

    pub fn to_string_fixed(&self) -> String {
        format_decimal_with_scale(self.amount, self.amount.scale() as usize)
    }

    pub fn to_f64(&self) -> Option<f64> {
        self.amount.to_f64()
    }
}

impl Serialize for CurrencyValue {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string_fixed())
    }
}

impl<'de> Deserialize<'de> for CurrencyValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let token = String::deserialize(deserializer)?;
        CurrencyValue::parse(&token).map_err(|err| de::Error::custom(err.to_string()))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Value {
    String(String),
    Integer(i64),
    Float(f64),
    Boolean(bool),
    Date(NaiveDate),
    DateTime(NaiveDateTime),
    Time(NaiveTime),
    Guid(Uuid),
    Decimal(FixedDecimalValue),
    Currency(CurrencyValue),
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
            Value::Time(t) => t.format("%H:%M:%S").to_string(),
            Value::Guid(g) => g.to_string(),
            Value::Decimal(d) => d.to_string_fixed(),
            Value::Currency(c) => c.to_string_fixed(),
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
            (Value::Time(a), Value::Time(b)) => a.cmp(b),
            (Value::Guid(a), Value::Guid(b)) => a.cmp(b),
            (Value::Decimal(a), Value::Decimal(b)) => a.cmp(b),
            (Value::Currency(a), Value::Currency(b)) => a.cmp(b),
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

pub fn parse_naive_time(value: &str) -> Result<NaiveTime> {
    const TIME_FORMATS: &[&str] = &["%H:%M:%S", "%H:%M"];
    for fmt in TIME_FORMATS {
        if let Ok(parsed) = NaiveTime::parse_from_str(value, fmt) {
            return Ok(parsed);
        }
    }
    Err(anyhow!("Failed to parse '{value}' as time"))
}

pub fn normalize_column_name(name: &str) -> String {
    let mut normalized: String = name
        .chars()
        .map(|c| match c {
            'a'..='z' | 'A'..='Z' | '0'..='9' => c,
            _ => '_',
        })
        .collect();

    if normalized.is_empty() {
        normalized.push_str("column");
    }

    if normalized
        .chars()
        .next()
        .is_none_or(|c| !(c.is_ascii_alphabetic() || c == '_'))
    {
        normalized.insert(0, '_');
    }

    normalized.to_ascii_lowercase()
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
        ColumnType::Time => {
            let parsed = parse_naive_time(value)?;
            Value::Time(parsed)
        }
        ColumnType::Guid => {
            let trimmed = value.trim().trim_matches(|c| matches!(c, '{' | '}'));
            let parsed = Uuid::parse_str(trimmed)
                .with_context(|| format!("Failed to parse '{value}' as GUID"))?;
            Value::Guid(parsed)
        }
        ColumnType::Decimal(spec) => {
            let parsed = FixedDecimalValue::parse(value, spec)?;
            Value::Decimal(parsed)
        }
        ColumnType::Currency => {
            let parsed = CurrencyValue::parse(value)?;
            Value::Currency(parsed)
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
        Value::Time(t) => evalexpr::Value::String(t.format("%H:%M:%S").to_string()),
        Value::Guid(g) => evalexpr::Value::String(g.to_string()),
        Value::Decimal(d) => d
            .to_f64()
            .map(evalexpr::Value::Float)
            .unwrap_or_else(|| evalexpr::Value::String(d.to_string_fixed())),
        Value::Currency(c) => c
            .to_f64()
            .map(evalexpr::Value::Float)
            .unwrap_or_else(|| evalexpr::Value::String(c.to_string_fixed())),
    }
}

pub fn parse_decimal_literal(raw: &str) -> Result<Decimal> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        bail!("Decimal value is empty");
    }

    let mut negative = false;
    let mut body = trimmed;
    if body.starts_with('(') && body.ends_with(')') {
        negative = true;
        body = &body[1..body.len() - 1];
    }

    body = body.trim();
    if body.starts_with('-') {
        negative = true;
        body = &body[1..];
    } else if body.starts_with('+') {
        body = &body[1..];
    }

    body = body.trim();
    let mut sanitized = String::with_capacity(body.len() + 1);
    let mut decimal_seen = false;
    for ch in body.chars() {
        match ch {
            '0'..='9' => sanitized.push(ch),
            '.' => {
                if decimal_seen {
                    bail!("Decimal value '{raw}' contains multiple decimal points");
                }
                decimal_seen = true;
                sanitized.push(ch);
            }
            ',' | '_' | ' ' => {
                // Skip common thousands separators and spacing.
            }
            _ => {
                bail!("Decimal value '{raw}' contains unsupported character '{ch}'");
            }
        }
    }

    ensure!(
        sanitized.chars().any(|c| c.is_ascii_digit()),
        "Decimal value '{raw}' does not contain digits"
    );

    if negative {
        sanitized.insert(0, '-');
    }

    Decimal::from_str(&sanitized).with_context(|| format!("Parsing '{raw}' as decimal"))
}

fn format_decimal_with_scale(mut value: Decimal, scale: usize) -> String {
    let target_scale = scale as u32;
    if value.scale() < target_scale {
        value.rescale(target_scale);
    }
    if scale == 0 {
        let mut rendered = value.to_string();
        if let Some(idx) = rendered.find('.') {
            rendered.truncate(idx);
        }
        return rendered;
    }
    let rendered = value.to_string();
    let actual = rendered
        .split_once('.')
        .map(|(_, frac)| frac.len())
        .unwrap_or(0);
    if actual == scale {
        return rendered;
    }
    if let Some((whole, frac)) = rendered.split_once('.') {
        let mut buf = String::new();
        buf.push_str(whole);
        buf.push('.');
        buf.push_str(frac);
        for _ in 0..(scale.saturating_sub(actual)) {
            buf.push('0');
        }
        return buf;
    }
    let mut buf = String::new();
    buf.push_str(&rendered);
    buf.push('.');
    for _ in 0..scale {
        buf.push('0');
    }
    buf
}

fn count_integer_digits(decimal: &Decimal) -> usize {
    let abs = decimal.abs();
    if abs < Decimal::ONE {
        return 0;
    }
    abs.trunc()
        .to_string()
        .chars()
        .filter(|c| c.is_ascii_digit())
        .count()
}

pub fn parse_currency_decimal(raw: &str) -> Result<Decimal> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        bail!("Currency value is empty");
    }

    let mut negative = false;
    let mut body = trimmed;
    if body.starts_with('(') && body.ends_with(')') {
        negative = true;
        body = &body[1..body.len() - 1];
    }

    body = body.trim();
    if body.starts_with('-') {
        negative = true;
        body = &body[1..];
    } else if body.starts_with('+') {
        body = &body[1..];
    }

    body = body.trim();
    let mut sanitized = String::with_capacity(body.len() + 1);
    let mut decimal_seen = false;
    for ch in body.chars() {
        match ch {
            '0'..='9' => sanitized.push(ch),
            '.' => {
                if decimal_seen {
                    bail!("Currency value '{raw}' contains multiple decimal points");
                }
                decimal_seen = true;
                sanitized.push(ch);
            }
            ',' | '_' | ' ' => {
                // Skip common thousands separators and spacing.
            }
            '$' | '€' | '£' | '¥' => {
                // Skip well-known currency symbols.
            }
            _ => {
                bail!("Currency value '{raw}' contains unsupported character '{ch}'");
            }
        }
    }

    ensure!(
        sanitized.chars().any(|c| c.is_ascii_digit()),
        "Currency value '{raw}' does not contain digits"
    );

    if negative {
        sanitized.insert(0, '-');
    }

    Decimal::from_str(&sanitized).with_context(|| format!("Parsing '{raw}' as decimal"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::{ColumnType, DecimalSpec};
    use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
    use evalexpr::Value as EvalValue;
    use rust_decimal::Decimal;
    use std::str::FromStr;
    use uuid::Uuid;

    #[test]
    fn normalize_column_name_replaces_non_alphanumeric() {
        assert_eq!(normalize_column_name("Order ID"), "order_id");
        assert_eq!(normalize_column_name("$Percent%"), "_percent_");
        assert_eq!(normalize_column_name("123Metric"), "_123metric");
        assert_eq!(normalize_column_name(""), "column");
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
    fn parse_naive_time_supports_multiple_formats() {
        let expected = NaiveTime::from_hms_opt(14, 30, 0).unwrap();
        assert_eq!(parse_naive_time("14:30:00").unwrap(), expected);
        assert_eq!(parse_naive_time("14:30").unwrap(), expected);
        assert!(parse_naive_time("24:61").is_err());
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

    #[test]
    fn parse_currency_values_accepts_two_and_four_decimals() {
        let two = parse_typed_value("$1,234.56", &ColumnType::Currency)
            .unwrap()
            .unwrap();
        let four = parse_typed_value("123.4567", &ColumnType::Currency)
            .unwrap()
            .unwrap();
        match (two, four) {
            (Value::Currency(a), Value::Currency(b)) => {
                assert_eq!(a.scale(), 2);
                assert_eq!(a.to_string_fixed(), "1234.56");
                assert_eq!(b.scale(), 4);
                assert_eq!(b.to_string_fixed(), "123.4567");
            }
            _ => panic!("Expected currency values"),
        }
    }

    #[test]
    fn parse_currency_rejects_invalid_precision() {
        assert!(parse_typed_value("1.234", &ColumnType::Currency).is_err());
        assert!(parse_typed_value("abc", &ColumnType::Currency).is_err());
    }

    #[test]
    fn parse_currency_rejects_embedded_letters() {
        let err = parse_typed_value("12a.34", &ColumnType::Currency)
            .expect_err("currency parser should reject embedded letters");
        assert!(err.to_string().contains("contains unsupported character"));
    }

    #[test]
    fn currency_quantize_rounds_half_away_from_zero() {
        let decimal = Decimal::from_str("10.005").unwrap();
        let value = CurrencyValue::quantize(decimal, 2, None).expect("round currency");
        assert_eq!(value.to_string_fixed(), "10.01");
    }

    #[test]
    fn currency_quantize_truncates_values() {
        let decimal = Decimal::from_str("7.899").unwrap();
        let value =
            CurrencyValue::quantize(decimal, 2, Some("truncate")).expect("truncate currency");
        assert_eq!(value.to_string_fixed(), "7.89");
    }

    #[test]
    fn currency_quantize_truncates_four_decimal_precision() {
        let decimal = Decimal::from_str("1.234567").unwrap();
        let value =
            CurrencyValue::quantize(decimal, 4, Some("truncate")).expect("truncate currency");
        assert_eq!(value.to_string_fixed(), "1.2345");
    }

    #[test]
    fn currency_quantize_rejects_invalid_strategy() {
        let decimal = Decimal::from_str("1.00").unwrap();
        assert!(CurrencyValue::quantize(decimal, 2, Some("ceil")).is_err());
    }

    #[test]
    fn currency_quantize_rejects_invalid_scale() {
        let decimal = Decimal::from_str("1.00").unwrap();
        assert!(CurrencyValue::quantize(decimal, 3, None).is_err());
    }

    #[test]
    fn currency_to_string_fixed_pads_fractional_zeros() {
        let value = CurrencyValue::parse("42").expect("parse integer currency");
        assert_eq!(value.to_string_fixed(), "42.00");
    }

    #[test]
    fn fixed_decimal_value_truncate_strategy_respects_scale() {
        let spec = DecimalSpec::new(8, 2).expect("valid decimal spec");
        let decimal = Decimal::from_str("123.456").expect("valid decimal literal");
        let value = FixedDecimalValue::from_decimal(decimal, &spec, Some("truncate"))
            .expect("truncate decimal");
        assert_eq!(value.to_string_fixed(), "123.45");
        assert_eq!(value.scale(), 2);
    }

    #[test]
    fn fixed_decimal_value_round_strategy_respects_scale() {
        let spec = DecimalSpec::new(10, 3).expect("valid decimal spec");
        let decimal = Decimal::from_str("-87.6549").expect("valid decimal literal");
        let value =
            FixedDecimalValue::from_decimal(decimal, &spec, Some("round")).expect("round decimal");
        assert_eq!(value.to_string_fixed(), "-87.655");
        assert_eq!(value.scale(), 3);
    }

    #[test]
    fn fixed_decimal_value_rejects_precision_overflow() {
        let spec = DecimalSpec::new(6, 2).expect("valid decimal spec");
        let decimal = Decimal::from_str("12345.67").expect("decimal literal");
        let err =
            FixedDecimalValue::from_decimal(decimal, &spec, None).expect_err("precision overflow");
        assert!(err.to_string().contains("must not exceed"));
    }

    #[test]
    fn fixed_decimal_value_rescales_short_fractional_parts() {
        let spec = DecimalSpec::new(12, 4).expect("valid decimal spec");
        let decimal = Decimal::from_str("42").expect("whole number decimal");
        let value = FixedDecimalValue::from_decimal(decimal, &spec, None).expect("rescale decimal");
        assert_eq!(value.to_string_fixed(), "42.0000");
        assert_eq!(value.scale(), 4);
    }

    #[test]
    fn parse_decimal_literal_supports_parentheses_and_separators() {
        let parsed = parse_decimal_literal("(1,234.50)").expect("parse negative grouped decimal");
        assert_eq!(parsed, Decimal::from_str("-1234.50").unwrap());
    }

    #[test]
    fn parse_decimal_literal_supports_positive_sign_and_underscores() {
        let parsed = parse_decimal_literal(" +7_654.321 ").expect("parse underscored decimal");
        assert_eq!(parsed, Decimal::from_str("7654.321").unwrap());
    }

    #[test]
    fn parse_decimal_literal_rejects_invalid_characters() {
        assert!(parse_decimal_literal("12a.34").is_err());
        assert!(parse_decimal_literal("#42.0").is_err());
    }

    #[test]
    fn parse_decimal_literal_rejects_multiple_decimal_points() {
        assert!(parse_decimal_literal("1.2.3").is_err());
    }

    #[test]
    fn parse_decimal_values_enforce_precision_and_scale() {
        let spec = DecimalSpec::new(10, 4).expect("valid decimal spec");
        let decimal_type = ColumnType::Decimal(spec.clone());
        let parsed = parse_typed_value("123.4567", &decimal_type)
            .expect("parse decimal")
            .expect("non-empty decimal");
        match parsed {
            Value::Decimal(value) => {
                assert_eq!(value.scale(), 4);
                assert_eq!(value.precision(), 10);
                assert_eq!(value.to_string_fixed(), "123.4567");
            }
            other => panic!("Expected decimal value, got {other:?}"),
        }

        let narrow_spec = DecimalSpec::new(6, 2).expect("valid decimal spec");
        let narrow_type = ColumnType::Decimal(narrow_spec);
        assert!(parse_typed_value("123.456", &narrow_type).is_err());
        assert!(parse_typed_value("1234567", &narrow_type).is_err());
    }

    // -----------------------------------------------------------------------
    // FR-013: Boolean parsing — all 6 input format pairs
    // -----------------------------------------------------------------------

    #[test]
    fn parse_boolean_accepts_all_six_truthy_formats() {
        for input in &["true", "True", "t", "T", "yes", "Yes", "y", "Y", "1"] {
            let result = parse_typed_value(input, &ColumnType::Boolean)
                .unwrap_or_else(|_| panic!("should parse '{input}' as boolean"))
                .expect("non-empty");
            assert_eq!(
                result,
                Value::Boolean(true),
                "input '{input}' should be true"
            );
        }
    }

    #[test]
    fn parse_boolean_accepts_all_six_falsy_formats() {
        for input in &["false", "False", "f", "F", "no", "No", "n", "N", "0"] {
            let result = parse_typed_value(input, &ColumnType::Boolean)
                .unwrap_or_else(|_| panic!("should parse '{input}' as boolean"))
                .expect("non-empty");
            assert_eq!(
                result,
                Value::Boolean(false),
                "input '{input}' should be false"
            );
        }
    }

    // -----------------------------------------------------------------------
    // FR-014: Date parsing — failure path
    // -----------------------------------------------------------------------

    #[test]
    fn parse_naive_date_rejects_invalid_input() {
        assert!(parse_naive_date("not-a-date").is_err());
        assert!(parse_naive_date("2024-13-01").is_err());
        assert!(parse_naive_date("").is_err());
    }

    #[test]
    fn parse_naive_datetime_rejects_invalid_input() {
        assert!(parse_naive_datetime("not-a-datetime").is_err());
        assert!(parse_naive_datetime("2024-01-01 25:00:00").is_err());
        assert!(parse_naive_datetime("").is_err());
    }

    // -----------------------------------------------------------------------
    // FR-015: Currency parsing — symbol coverage
    // -----------------------------------------------------------------------

    #[test]
    fn parse_currency_accepts_all_supported_symbols() {
        for (raw, expected) in [
            ("$100.00", "100.00"),
            ("€200.50", "200.50"),
            ("£300.75", "300.75"),
            ("¥400.25", "400.25"),
        ] {
            let parsed = parse_typed_value(raw, &ColumnType::Currency)
                .unwrap_or_else(|_| panic!("should parse '{raw}'"))
                .expect("non-empty");
            match parsed {
                Value::Currency(c) => {
                    assert_eq!(c.to_string_fixed(), expected, "symbol input '{raw}'")
                }
                other => panic!("Expected currency for '{raw}', got {other:?}"),
            }
        }
    }

    #[test]
    fn parse_currency_accepts_parentheses_negative() {
        let parsed = parse_typed_value("($500.00)", &ColumnType::Currency)
            .expect("parse parenthesized currency")
            .expect("non-empty");
        match parsed {
            Value::Currency(c) => assert_eq!(c.to_string_fixed(), "-500.00"),
            other => panic!("Expected currency, got {other:?}"),
        }
    }
}
