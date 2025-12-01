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
