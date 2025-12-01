use std::{
    borrow::Cow,
    collections::{BTreeMap, HashSet},
    fmt,
    path::Path,
    str::FromStr,
};

use anyhow::{Context, Result, anyhow, bail, ensure};
use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use encoding_rs::Encoding;
use rust_decimal::Decimal;
use rust_decimal::prelude::FromPrimitive;
use serde::{Deserialize, Deserializer, Serialize, Serializer, de};
use serde_yaml::Value;
use uuid::Uuid;

use crate::{
    data::{
        CurrencyValue, FixedDecimalValue, Value as DataValue, parse_currency_decimal,
        parse_decimal_literal, parse_naive_date, parse_naive_datetime, parse_naive_time,
        parse_typed_value,
    },
    io_utils,
    yaml_provider::{self, YamlValue},
};

const DECIMAL_MAX_PRECISION: u32 = 28;
const HEADER_ALIAS_THRESHOLD_PERCENT: usize = 80;
const HEADER_ALIAS_MIN_MATCHES: usize = 4;
const HEADER_DETECTION_SAMPLE_ROWS: usize = 6;

const COMMON_HEADER_TOKENS: &[&str] = &[
    "address",
    "amount",
    "category",
    "city",
    "code",
    "country",
    "created",
    "currency",
    "date",
    "description",
    "email",
    "first_name",
    "id",
    "item",
    "last_name",
    "name",
    "phone",
    "price",
    "quantity",
    "state",
    "status",
    "total",
    "type",
    "updated",
    "zip",
];

#[derive(Debug, Clone)]
pub struct CsvLayout {
    pub headers: Vec<String>,
    pub has_headers: bool,
}

impl CsvLayout {
    pub fn field_count(&self) -> usize {
        self.headers.len()
    }
}

#[derive(Debug, Clone, Default)]
pub enum PlaceholderPolicy {
    #[default]
    TreatAsEmpty,
    FillWith(String),
}

#[derive(Debug, Clone, Default)]
pub struct PlaceholderSummary {
    counts: BTreeMap<String, usize>,
}

impl PlaceholderSummary {
    pub fn is_empty(&self) -> bool {
        self.counts.is_empty()
    }

    pub fn record(&mut self, value: &str) {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return;
        }
        *self.counts.entry(trimmed.to_string()).or_insert(0) += 1;
    }

    pub fn entries(&self) -> Vec<(String, usize)> {
        self.counts
            .iter()
            .map(|(token, count)| (token.clone(), *count))
            .collect()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DecimalSpec {
    pub precision: u32,
    pub scale: u32,
}

impl DecimalSpec {
    pub fn new(precision: u32, scale: u32) -> Result<Self> {
        let spec = Self { precision, scale };
        spec.ensure_valid()?;
        Ok(spec)
    }

    pub fn ensure_valid(&self) -> Result<()> {
        ensure!(self.precision > 0, "Decimal precision must be positive");
        ensure!(
            self.precision <= DECIMAL_MAX_PRECISION,
            "Decimal precision must be <= {}",
            DECIMAL_MAX_PRECISION
        );
        ensure!(
            self.scale <= self.precision,
            "Decimal scale ({}) cannot exceed precision ({})",
            self.scale,
            self.precision
        );
        ensure!(
            self.scale <= DECIMAL_MAX_PRECISION,
            "Decimal scale must be <= {}",
            DECIMAL_MAX_PRECISION
        );
        Ok(())
    }

    pub fn signature(&self) -> String {
        format!("decimal({},{})", self.precision, self.scale)
    }

    pub fn describe(&self) -> String {
        format!("decimal(precision={},scale={})", self.precision, self.scale)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ColumnType {
    String,
    Integer,
    Float,
    Boolean,
    Date,
    DateTime,
    Time,
    Guid,
    Currency,
    Decimal(DecimalSpec),
}

impl Serialize for ColumnType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            ColumnType::String => serializer.serialize_str("String"),
            ColumnType::Integer => serializer.serialize_str("Integer"),
            ColumnType::Float => serializer.serialize_str("Float"),
            ColumnType::Boolean => serializer.serialize_str("Boolean"),
            ColumnType::Date => serializer.serialize_str("Date"),
            ColumnType::DateTime => serializer.serialize_str("DateTime"),
            ColumnType::Time => serializer.serialize_str("Time"),
            ColumnType::Guid => serializer.serialize_str("Guid"),
            ColumnType::Currency => serializer.serialize_str("Currency"),
            ColumnType::Decimal(spec) => serializer.serialize_str(&spec.signature()),
        }
    }
}

impl<'de> Deserialize<'de> for ColumnType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let human_readable = deserializer.is_human_readable();
        #[cfg(test)]
        {
            if !human_readable && std::env::var("CSV_MANAGED_DEBUG_COLUMN_TYPE").is_ok() {
                eprintln!("ColumnType binary deserialize invoked");
            }
        }
        if human_readable {
            let value = YamlValue::deserialize(deserializer)?;
            parse_human_readable_column_type(value).map_err(de::Error::custom)
        } else {
            let token = String::deserialize(deserializer)?;
            ColumnType::from_str(&token).map_err(|err| de::Error::custom(err.to_string()))
        }
    }
}

fn parse_decimal_from_mapping(value: YamlValue) -> Result<ColumnType> {
    let mapping = value
        .as_mapping()
        .ok_or_else(|| anyhow!("Decimal mapping must be a map with precision/scale"))?;

    let mut precision: Option<u32> = None;
    let mut scale: Option<u32> = None;

    for (key, val) in mapping {
        let key_str = key
            .as_str()
            .ok_or_else(|| anyhow!("Decimal mapping keys must be strings"))?
            .to_ascii_lowercase();

        match key_str.as_str() {
            "precision" => {
                let parsed = val
                    .as_u64()
                    .ok_or_else(|| anyhow!("Decimal precision must be an unsigned integer"))?;
                precision = Some(parsed as u32);
            }
            "scale" => {
                let parsed = val
                    .as_u64()
                    .ok_or_else(|| anyhow!("Decimal scale must be an unsigned integer"))?;
                scale = Some(parsed as u32);
            }
            other => {
                return Err(anyhow!("Unknown decimal key '{other}'"));
            }
        }
    }

    let precision = precision.ok_or_else(|| anyhow!("Decimal mapping requires precision"))?;
    let scale = scale.ok_or_else(|| anyhow!("Decimal mapping requires scale"))?;
    let spec = DecimalSpec::new(precision, scale)?;
    Ok(ColumnType::Decimal(spec))
}

fn parse_human_readable_column_type(value: YamlValue) -> Result<ColumnType> {
    if let Some(token) = value.as_str() {
        return ColumnType::from_str(token);
    }

    if let Some(mapping) = value.as_mapping()
        && mapping.len() == 1
        && let Some((key, val)) = mapping.iter().next()
    {
        let key_normalized = key
            .as_str()
            .ok_or_else(|| anyhow!("Structured datatype key must be a string"))?
            .trim()
            .to_ascii_lowercase();
        return match key_normalized.as_str() {
            "decimal" => parse_decimal_from_mapping(val.clone()),
            other => Err(anyhow!("Unsupported structured datatype '{other}'")),
        };
    }

    Err(anyhow!(
        "Unsupported column datatype representation: {value:?}"
    ))
}

impl ColumnType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ColumnType::String => "string",
            ColumnType::Integer => "integer",
            ColumnType::Float => "float",
            ColumnType::Boolean => "boolean",
            ColumnType::Date => "date",
            ColumnType::DateTime => "datetime",
            ColumnType::Time => "time",
            ColumnType::Guid => "guid",
            ColumnType::Currency => "currency",
            ColumnType::Decimal(_) => "decimal",
        }
    }

    pub fn variants() -> &'static [&'static str] {
        &[
            "string",
            "integer",
            "float",
            "boolean",
            "date",
            "datetime",
            "time",
            "guid",
            "currency",
            "decimal(precision,scale)",
        ]
    }

    pub fn describe(&self) -> String {
        match self {
            ColumnType::Decimal(spec) => spec.describe(),
            _ => self.as_str().to_string(),
        }
    }

    pub fn signature_token(&self) -> String {
        match self {
            ColumnType::Decimal(spec) => spec.signature(),
            _ => self.as_str().to_string(),
        }
    }

    pub fn cli_token(&self) -> String {
        match self {
            ColumnType::Decimal(spec) => format!("decimal({},{})", spec.precision, spec.scale),
            _ => self.as_str().to_string(),
        }
    }

    pub fn decimal_spec(&self) -> Option<&DecimalSpec> {
        match self {
            ColumnType::Decimal(spec) => Some(spec),
            _ => None,
        }
    }
}

impl fmt::Display for ColumnType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.describe())
    }
}

impl FromStr for ColumnType {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let normalized = value.trim().to_ascii_lowercase();
        match normalized.as_str() {
            "string" => Ok(ColumnType::String),
            "integer" | "int" => Ok(ColumnType::Integer),
            "float" | "double" => Ok(ColumnType::Float),
            "boolean" | "bool" => Ok(ColumnType::Boolean),
            "date" => Ok(ColumnType::Date),
            "datetime" | "date-time" | "timestamp" => Ok(ColumnType::DateTime),
            "time" => Ok(ColumnType::Time),
            "guid" | "uuid" => Ok(ColumnType::Guid),
            "currency" => Ok(ColumnType::Currency),
            other if other.starts_with("decimal") => parse_decimal_type(value),
            _ => Err(anyhow!(
                "Unknown column type '{value}'. Supported types: {}",
                ColumnType::variants().join(", ")
            )),
        }
    }
}

fn parse_decimal_type(value: &str) -> Result<ColumnType> {
    let trimmed = value.trim();
    let start = trimmed.find('(').ok_or_else(|| {
        anyhow!("Decimal type must specify precision and scale, e.g. decimal(18,4)")
    })?;
    ensure!(
        trimmed.ends_with(')'),
        "Decimal type must close with ')', e.g. decimal(18,4)"
    );
    let inner = &trimmed[start + 1..trimmed.len() - 1];
    let mut precision: Option<u32> = None;
    let mut scale: Option<u32> = None;
    let mut positional = Vec::new();

    for part in inner.split(',') {
        let token = part.trim();
        if token.is_empty() {
            continue;
        }
        if let Some((key, value)) = token
            .split_once(['=', ':'])
            .map(|(k, v)| (k.trim(), v.trim()))
        {
            let key_normalized = key.to_ascii_lowercase();
            let parsed: u32 = value
                .parse()
                .with_context(|| format!("Parsing decimal {key}='{value}' in '{token}'"))?;
            match key_normalized.as_str() {
                "precision" => {
                    precision = Some(parsed);
                }
                "scale" => {
                    scale = Some(parsed);
                }
                other => {
                    bail!("Unknown decimal option '{other}' in '{token}'");
                }
            }
        } else {
            positional.push(token);
        }
    }

    if let Some(first) = positional.first()
        && precision.is_none()
    {
        precision =
            Some(first.parse().with_context(|| {
                format!("Parsing decimal precision from '{first}' in '{value}'")
            })?);
    }
    if let Some(second) = positional.get(1)
        && scale.is_none()
    {
        scale = Some(
            second
                .parse()
                .with_context(|| format!("Parsing decimal scale from '{second}' in '{value}'"))?,
        );
    }
    ensure!(
        positional.len() <= 2,
        "Decimal type accepts at most two positional arguments"
    );

    let precision = precision
        .ok_or_else(|| anyhow!("Decimal type requires a precision value, e.g. decimal(18,4)"))?;
    let scale =
        scale.ok_or_else(|| anyhow!("Decimal type requires a scale value, e.g. decimal(18,4)"))?;

    let spec = DecimalSpec::new(precision, scale)?;
    Ok(ColumnType::Decimal(spec))
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ValueReplacement {
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DatatypeMapping {
    pub from: ColumnType,
    pub to: ColumnType,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strategy: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub options: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnMeta {
    pub name: String,
    pub datatype: ColumnType,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "name_mapping"
    )]
    pub rename: Option<String>,
    #[serde(
        default,
        rename = "replace",
        alias = "value_replacements",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub value_replacements: Vec<ValueReplacement>,
    #[serde(
        default,
        rename = "datatype_mappings",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub datatype_mappings: Vec<DatatypeMapping>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Schema {
    pub columns: Vec<ColumnMeta>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schema_version: Option<String>,
    #[serde(default = "Schema::default_has_headers")]
    pub has_headers: bool,
}

#[derive(Debug, Clone)]
pub struct ColumnSummary {
    pub non_empty: usize,
    pub tracked_values: Vec<(String, usize)>,
    pub other_values: usize,
}

#[derive(Debug, Clone)]
pub struct InferenceStats {
    sample_values: Vec<Option<String>>,
    rows_read: usize,
    requested_rows: usize,
    decode_errors: usize,
    summaries: Vec<ColumnSummary>,
    placeholder_summaries: Vec<PlaceholderSummary>,
}

impl InferenceStats {
    pub fn sample_value(&self, index: usize) -> Option<&str> {
        self.sample_values
            .get(index)
            .and_then(|value| value.as_deref())
    }

    pub fn summary(&self, index: usize) -> Option<&ColumnSummary> {
        self.summaries.get(index)
    }

    pub fn rows_read(&self) -> usize {
        self.rows_read
    }

    pub fn requested_rows(&self) -> usize {
        self.requested_rows
    }

    pub fn decode_errors(&self) -> usize {
        self.decode_errors
    }

    pub fn placeholder_summary(&self, index: usize) -> Option<&PlaceholderSummary> {
        self.placeholder_summaries.get(index)
    }
}

impl Schema {
    pub fn from_headers(headers: &[String]) -> Self {
        let columns = headers
            .iter()
            .map(|name| ColumnMeta {
                name: name.clone(),
                datatype: ColumnType::String,
                rename: None,
                value_replacements: Vec::new(),
                datatype_mappings: Vec::new(),
            })
            .collect();
        Schema {
            columns,
            schema_version: None,
            has_headers: true,
        }
    }

    pub const fn default_has_headers() -> bool {
        true
    }

    pub fn expects_headers(&self) -> bool {
        self.has_headers
    }

    pub fn column_index(&self, name: &str) -> Option<usize> {
        self.columns
            .iter()
            .position(|c| c.name == name || c.rename.as_deref() == Some(name))
    }

    pub fn headers(&self) -> Vec<String> {
        self.columns.iter().map(|c| c.name.clone()).collect()
    }

    pub fn output_headers(&self) -> Vec<String> {
        self.columns
            .iter()
            .map(|c| c.output_name().to_string())
            .collect()
    }

    pub(crate) fn header_alias_sets(&self) -> Vec<HashSet<String>> {
        self.columns
            .iter()
            .map(|column| build_header_aliases(&column.name))
            .collect()
    }

    pub fn validate_headers(&self, headers: &[String]) -> Result<()> {
        if !self.has_headers {
            return Ok(());
        }
        if headers.len() != self.columns.len() {
            return Err(anyhow!(
                "Header length mismatch: schema expects {} column(s) but file contains {}",
                self.columns.len(),
                headers.len()
            ));
        }
        for (idx, column) in self.columns.iter().enumerate() {
            let name = headers.get(idx).map(|s| s.as_str()).unwrap_or_default();
            if column.matches_header(name) {
                continue;
            }
            if let Some(mapped) = column
                .rename
                .as_deref()
                .filter(|value| !value.is_empty() && *value != column.name)
            {
                if mapped.eq_ignore_ascii_case(name) {
                    continue;
                }
                return Err(anyhow!(
                    "Header mismatch at position {}: expected '{}' (or mapped '{}') but found '{}'",
                    idx + 1,
                    column.name,
                    mapped,
                    name
                ));
            }
            return Err(anyhow!(
                "Header mismatch at position {}: expected '{}' but found '{}'",
                idx + 1,
                column.name,
                name
            ));
        }
        Ok(())
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        self.save_internal(path, false)
    }

    pub fn save_with_replace_template(&self, path: &Path) -> Result<()> {
        self.save_internal(path, true)
    }

    pub fn to_yaml_string(&self, include_replace_template: bool) -> Result<String> {
        let value = self.to_yaml_value(include_replace_template)?;
        yaml_provider::to_string(&value)
    }

    pub fn load(path: &Path) -> Result<Self> {
        let schema: Schema = yaml_provider::load_from_path(path)?;
        schema.validate_datatype_mappings()?;
        Ok(schema)
    }

    fn save_internal(&self, path: &Path, include_replace_template: bool) -> Result<()> {
        let value = self.to_yaml_value(include_replace_template)?;
        yaml_provider::save_to_path(path, &value)
    }

    fn to_yaml_value(&self, include_replace_template: bool) -> Result<Value> {
        let mut schema = self.clone();
        if schema.schema_version.is_none() {
            schema.schema_version = Some(CURRENT_SCHEMA_VERSION.to_string());
        }
        schema.validate_datatype_mappings()?;

        let mut value: Value = yaml_provider::to_value(&schema)?;
        if include_replace_template
            && let Some(columns) = value
                .get_mut("columns")
                .and_then(|columns| columns.as_sequence_mut())
        {
            for column in columns {
                if let Some(obj) = column.as_mapping_mut() {
                    if let Some(existing) = obj.remove(Value::from("value_replacements")) {
                        obj.insert(Value::from("replace"), existing);
                    }
                    let replace_key = Value::from("replace");
                    if !obj.contains_key(&replace_key) {
                        obj.insert(replace_key, Value::Sequence(Vec::new()));
                    }
                }
            }
        }
        Ok(value)
    }
}

fn parse_initial_value(raw: &str, mapping: &DatatypeMapping) -> Result<DataValue> {
    match mapping.from {
        ColumnType::String => Ok(DataValue::String(raw.to_string())),
        _ => parse_with_type(raw, &mapping.from),
    }
}

fn parse_with_type(value: &str, ty: &ColumnType) -> Result<DataValue> {
    let trimmed = value.trim();
    parse_typed_value(trimmed, ty)
        .with_context(|| format!("Parsing '{trimmed}' as {ty}"))?
        .ok_or_else(|| anyhow!("Value is empty after trimming"))
}

fn value_column_type(value: &DataValue) -> ColumnType {
    match value {
        DataValue::String(_) => ColumnType::String,
        DataValue::Integer(_) => ColumnType::Integer,
        DataValue::Float(_) => ColumnType::Float,
        DataValue::Boolean(_) => ColumnType::Boolean,
        DataValue::Date(_) => ColumnType::Date,
        DataValue::DateTime(_) => ColumnType::DateTime,
        DataValue::Time(_) => ColumnType::Time,
        DataValue::Guid(_) => ColumnType::Guid,
        DataValue::Decimal(value) => ColumnType::Decimal(
            DecimalSpec::new(value.precision(), value.scale())
                .expect("FixedDecimalValue guarantees valid decimal spec"),
        ),
        DataValue::Currency(_) => ColumnType::Currency,
    }
}

fn apply_single_mapping(mapping: &DatatypeMapping, value: DataValue) -> Result<DataValue> {
    let strategy = normalized_strategy(mapping);
    match (&mapping.to, value) {
        (ColumnType::String, DataValue::String(mut s)) => {
            if let Some(strategy) = strategy.as_deref() {
                match strategy {
                    "trim" => s = s.trim().to_string(),
                    "lowercase" => s = s.to_ascii_lowercase(),
                    "uppercase" => s = s.to_ascii_uppercase(),
                    other => {
                        bail!("Strategy '{other}' is not valid for string -> string mappings");
                    }
                }
            }
            Ok(DataValue::String(s))
        }
        (ColumnType::String, DataValue::Integer(i)) => Ok(DataValue::String(i.to_string())),
        (ColumnType::String, DataValue::Float(f)) => {
            let scale = resolve_scale(mapping);
            let formatted =
                if strategy.as_deref() == Some("round") || mapping.from == ColumnType::Float {
                    format_float_with_scale(f, scale)
                } else {
                    f.to_string()
                };
            Ok(DataValue::String(formatted))
        }
        (ColumnType::String, DataValue::Boolean(b)) => Ok(DataValue::String(b.to_string())),
        (ColumnType::String, DataValue::Date(d)) => {
            let fmt = mapping
                .options
                .get("format")
                .and_then(|v| v.as_str())
                .unwrap_or("%Y-%m-%d");
            Ok(DataValue::String(d.format(fmt).to_string()))
        }
        (ColumnType::String, DataValue::DateTime(dt)) => {
            let fmt = mapping
                .options
                .get("format")
                .and_then(|v| v.as_str())
                .unwrap_or("%Y-%m-%d %H:%M:%S");
            Ok(DataValue::String(dt.format(fmt).to_string()))
        }
        (ColumnType::String, DataValue::Time(t)) => {
            let fmt = mapping
                .options
                .get("format")
                .and_then(|v| v.as_str())
                .unwrap_or("%H:%M:%S");
            Ok(DataValue::String(t.format(fmt).to_string()))
        }
        (ColumnType::String, DataValue::Guid(g)) => Ok(DataValue::String(g.to_string())),
        (ColumnType::String, DataValue::Decimal(d)) => Ok(DataValue::String(d.to_string_fixed())),
        (ColumnType::String, DataValue::Currency(c)) => Ok(DataValue::String(c.to_string_fixed())),
        (ColumnType::Integer, DataValue::String(s)) => {
            let parsed = parse_with_type(&s, &ColumnType::Integer)?;
            if let DataValue::Integer(i) = parsed {
                Ok(DataValue::Integer(i))
            } else {
                unreachable!()
            }
        }
        (ColumnType::Float, DataValue::String(s)) => {
            let parsed = parse_with_type(&s, &ColumnType::Float)?;
            let mut value = match parsed {
                DataValue::Float(f) => f,
                _ => unreachable!(),
            };
            if should_round_float(mapping, strategy.as_deref()) {
                value = round_float(value, resolve_scale(mapping));
            }
            Ok(DataValue::Float(value))
        }
        (ColumnType::Currency, DataValue::String(s)) => {
            let decimal = parse_currency_decimal(&s)?;
            let scale = explicit_currency_scale(mapping)?
                .unwrap_or_else(|| default_currency_scale(&decimal));
            let currency = CurrencyValue::quantize(decimal, scale, strategy.as_deref())?;
            Ok(DataValue::Currency(currency))
        }
        (ColumnType::Decimal(spec), DataValue::String(s)) => {
            let decimal = parse_decimal_literal(&s)?;
            let fixed = FixedDecimalValue::from_decimal(decimal, spec, strategy.as_deref())?;
            Ok(DataValue::Decimal(fixed))
        }
        (ColumnType::Boolean, DataValue::String(s)) => {
            let parsed = parse_with_type(&s, &ColumnType::Boolean)?;
            if let DataValue::Boolean(b) = parsed {
                Ok(DataValue::Boolean(b))
            } else {
                unreachable!()
            }
        }
        (ColumnType::Date, DataValue::String(s)) => {
            let parsed = parse_string_to_date(&s, mapping)?;
            Ok(DataValue::Date(parsed))
        }
        (ColumnType::DateTime, DataValue::String(s)) => {
            let parsed = parse_string_to_datetime(&s, mapping)?;
            Ok(DataValue::DateTime(parsed))
        }
        (ColumnType::Time, DataValue::String(s)) => {
            let parsed = parse_string_to_time(&s, mapping)?;
            Ok(DataValue::Time(parsed))
        }
        (ColumnType::Guid, DataValue::String(s)) => {
            let parsed = parse_with_type(&s, &ColumnType::Guid)?;
            if let DataValue::Guid(g) = parsed {
                Ok(DataValue::Guid(g))
            } else {
                unreachable!()
            }
        }
        (ColumnType::Date, DataValue::DateTime(dt)) => Ok(DataValue::Date(dt.date())),
        (ColumnType::Time, DataValue::DateTime(dt)) => Ok(DataValue::Time(dt.time())),
        (ColumnType::Float, DataValue::Integer(i)) => {
            let mut value = i as f64;
            if should_round_float(mapping, strategy.as_deref()) {
                value = round_float(value, resolve_scale(mapping));
            }
            Ok(DataValue::Float(value))
        }
        (ColumnType::Currency, DataValue::Integer(i)) => {
            let decimal = Decimal::from(i);
            let scale = explicit_currency_scale(mapping)?
                .unwrap_or_else(|| default_currency_scale(&decimal));
            let currency = CurrencyValue::quantize(decimal, scale, strategy.as_deref())?;
            Ok(DataValue::Currency(currency))
        }
        (ColumnType::Decimal(spec), DataValue::Integer(i)) => {
            let decimal = Decimal::from(i);
            let fixed = FixedDecimalValue::from_decimal(decimal, spec, strategy.as_deref())?;
            Ok(DataValue::Decimal(fixed))
        }
        (ColumnType::Integer, DataValue::Float(f)) => {
            let rounded = match strategy.as_deref() {
                Some("truncate") => f.trunc() as i64,
                _ => f.round() as i64,
            };
            Ok(DataValue::Integer(rounded))
        }
        (ColumnType::Float, DataValue::Float(f)) => {
            let mut value = f;
            if should_round_float(mapping, strategy.as_deref()) {
                value = round_float(value, resolve_scale(mapping));
            }
            Ok(DataValue::Float(value))
        }
        (ColumnType::Currency, DataValue::Float(f)) => {
            let decimal = Decimal::from_f64(f)
                .ok_or_else(|| anyhow!("Failed to convert float {f} to decimal"))?;
            let scale = explicit_currency_scale(mapping)?
                .unwrap_or_else(|| default_currency_scale(&decimal));
            let currency = CurrencyValue::quantize(decimal, scale, strategy.as_deref())?;
            Ok(DataValue::Currency(currency))
        }
        (ColumnType::Decimal(spec), DataValue::Float(f)) => {
            let decimal = Decimal::from_f64(f)
                .ok_or_else(|| anyhow!("Failed to convert float {f} to decimal"))?;
            let fixed = FixedDecimalValue::from_decimal(decimal, spec, strategy.as_deref())?;
            Ok(DataValue::Decimal(fixed))
        }
        (ColumnType::Float, DataValue::Currency(c)) => {
            let value = c
                .to_f64()
                .ok_or_else(|| anyhow!("Currency value out of f64 range"))?;
            Ok(DataValue::Float(value))
        }
        (ColumnType::Integer, DataValue::Currency(c)) => {
            let f = c
                .to_f64()
                .ok_or_else(|| anyhow!("Currency value out of range for integer conversion"))?;
            let rounded = match strategy.as_deref() {
                Some("truncate") => f.trunc() as i64,
                _ => f.round() as i64,
            };
            Ok(DataValue::Integer(rounded))
        }
        (ColumnType::Currency, DataValue::Currency(c)) => {
            let decimal = *c.amount();
            let scale = explicit_currency_scale(mapping)?
                .unwrap_or_else(|| default_currency_scale(&decimal));
            let currency = CurrencyValue::quantize(decimal, scale, strategy.as_deref())?;
            Ok(DataValue::Currency(currency))
        }
        (ColumnType::Decimal(spec), DataValue::Currency(c)) => {
            let fixed = FixedDecimalValue::from_decimal(*c.amount(), spec, strategy.as_deref())?;
            Ok(DataValue::Decimal(fixed))
        }
        (ColumnType::Float, DataValue::Decimal(d)) => {
            let value = d
                .to_f64()
                .ok_or_else(|| anyhow!("Decimal value out of f64 range"))?;
            Ok(DataValue::Float(value))
        }
        (ColumnType::Integer, DataValue::Decimal(d)) => {
            let value = d
                .to_f64()
                .ok_or_else(|| anyhow!("Decimal value out of range for integer conversion"))?;
            let rounded = match strategy.as_deref() {
                Some("truncate") => value.trunc() as i64,
                _ => value.round() as i64,
            };
            Ok(DataValue::Integer(rounded))
        }
        (ColumnType::Currency, DataValue::Decimal(d)) => {
            let decimal = *d.amount();
            let scale = explicit_currency_scale(mapping)?
                .unwrap_or_else(|| default_currency_scale(&decimal));
            let currency = CurrencyValue::quantize(decimal, scale, strategy.as_deref())?;
            Ok(DataValue::Currency(currency))
        }
        (ColumnType::Decimal(spec), DataValue::Decimal(existing)) => {
            if existing.precision() == spec.precision && existing.scale() == spec.scale {
                Ok(DataValue::Decimal(existing))
            } else {
                let fixed =
                    FixedDecimalValue::from_decimal(*existing.amount(), spec, strategy.as_deref())?;
                Ok(DataValue::Decimal(fixed))
            }
        }
        (ColumnType::Integer, DataValue::Integer(i)) => Ok(DataValue::Integer(i)),
        _ => bail!(
            "Datatype mapping '{}' -> '{}' is not supported",
            mapping.from,
            mapping.to
        ),
    }
}

fn render_mapped_value(value: &DataValue, mapping: &DatatypeMapping) -> Result<String> {
    match (&mapping.to, value) {
        (ColumnType::String, DataValue::String(s)) => Ok(s.clone()),
        (ColumnType::Integer, DataValue::Integer(i)) => Ok(i.to_string()),
        (ColumnType::Float, DataValue::Float(f)) => {
            let scale = resolve_scale(mapping);
            Ok(format_float_with_scale(*f, scale))
        }
        (ColumnType::Boolean, DataValue::Boolean(b)) => Ok(b.to_string()),
        (ColumnType::Date, DataValue::Date(d)) => {
            let fmt = mapping
                .options
                .get("format")
                .and_then(|v| v.as_str())
                .unwrap_or("%Y-%m-%d");
            Ok(d.format(fmt).to_string())
        }
        (ColumnType::DateTime, DataValue::DateTime(dt)) => {
            let fmt = mapping
                .options
                .get("format")
                .and_then(|v| v.as_str())
                .unwrap_or("%Y-%m-%d %H:%M:%S");
            Ok(dt.format(fmt).to_string())
        }
        (ColumnType::Time, DataValue::Time(t)) => {
            let fmt = mapping
                .options
                .get("format")
                .and_then(|v| v.as_str())
                .unwrap_or("%H:%M:%S");
            Ok(t.format(fmt).to_string())
        }
        (ColumnType::Guid, DataValue::Guid(g)) => Ok(g.to_string()),
        (ColumnType::Currency, DataValue::Currency(c)) => Ok(c.to_string_fixed()),
        (ColumnType::Decimal(spec), DataValue::Decimal(d)) => {
            if d.scale() == spec.scale && d.precision() == spec.precision {
                Ok(d.to_string_fixed())
            } else {
                let fixed = FixedDecimalValue::from_decimal(*d.amount(), spec, None)?;
                Ok(fixed.to_string_fixed())
            }
        }
        _ => bail!(
            "Mapping output type '{:?}' is incompatible with computed value '{:?}'",
            mapping.to,
            value_column_type(value)
        ),
    }
}

fn format_float_with_scale(value: f64, scale: usize) -> String {
    if scale == 0 {
        format!("{value:.0}")
    } else {
        format!("{:.precision$}", value, precision = scale)
    }
}

fn should_round_float(mapping: &DatatypeMapping, strategy: Option<&str>) -> bool {
    match strategy {
        Some("round") => true,
        Some(_) => false,
        None => mapping.from == ColumnType::Float && mapping.to == ColumnType::Float,
    }
}

fn round_float(value: f64, scale: usize) -> f64 {
    if scale == 0 {
        value.round()
    } else {
        let factor = 10f64.powi(scale as i32);
        (value * factor).round() / factor
    }
}

fn resolve_scale(mapping: &DatatypeMapping) -> usize {
    mapping
        .options
        .get("scale")
        .and_then(|value| {
            value
                .as_u64()
                .map(|u| u as usize)
                .or_else(|| value.as_i64().map(|i| i.max(0) as usize))
        })
        .unwrap_or(4)
}

fn explicit_currency_scale(mapping: &DatatypeMapping) -> Result<Option<u32>> {
    if let Some(scale) = mapping.options.get("scale") {
        let numeric = if let Some(value) = scale.as_u64() {
            value
        } else if let Some(value) = scale.as_i64() {
            ensure!(value >= 0, "Currency scale must be non-negative");
            value as u64
        } else {
            bail!("Currency scale must be numeric");
        };
        let scale_u32 = numeric as u32;
        ensure!(
            crate::data::CURRENCY_ALLOWED_SCALES.contains(&scale_u32),
            "Currency scale must be 2 or 4"
        );
        Ok(Some(scale_u32))
    } else {
        Ok(None)
    }
}

fn default_currency_scale(decimal: &Decimal) -> u32 {
    let scale = decimal.scale();
    if scale == 0 {
        2
    } else if crate::data::CURRENCY_ALLOWED_SCALES.contains(&scale) {
        scale
    } else if scale > 4 {
        4
    } else {
        2
    }
}

fn parse_string_to_date(value: &str, mapping: &DatatypeMapping) -> Result<NaiveDate> {
    let trimmed = value.trim();
    if let Some(fmt) = mapping.options.get("format").and_then(|v| v.as_str()) {
        NaiveDate::parse_from_str(trimmed, fmt)
            .with_context(|| format!("Parsing '{trimmed}' with format '{fmt}'"))
    } else {
        parse_naive_date(trimmed)
    }
}

fn parse_string_to_datetime(value: &str, mapping: &DatatypeMapping) -> Result<NaiveDateTime> {
    let trimmed = value.trim();
    if let Some(fmt) = mapping.options.get("format").and_then(|v| v.as_str()) {
        NaiveDateTime::parse_from_str(trimmed, fmt)
            .with_context(|| format!("Parsing '{trimmed}' with format '{fmt}'"))
    } else {
        parse_naive_datetime(trimmed)
    }
}

fn parse_string_to_time(value: &str, mapping: &DatatypeMapping) -> Result<NaiveTime> {
    let trimmed = value.trim();
    if let Some(fmt) = mapping.options.get("format").and_then(|v| v.as_str()) {
        NaiveTime::parse_from_str(trimmed, fmt)
            .with_context(|| format!("Parsing '{trimmed}' with format '{fmt}'"))
    } else {
        parse_naive_time(trimmed)
    }
}

fn normalized_strategy(mapping: &DatatypeMapping) -> Option<String> {
    mapping
        .strategy
        .as_ref()
        .map(|s| s.trim().to_ascii_lowercase())
        .filter(|s| !s.is_empty())
}

fn validate_mapping_options(column_name: &str, mapping: &DatatypeMapping) -> Result<()> {
    if let Some(strategy_raw) = mapping.strategy.as_ref() {
        let strategy = strategy_raw.trim();
        if !strategy.is_empty() {
            let normalized = strategy.to_ascii_lowercase();
            match normalized.as_str() {
                "round" | "trim" | "lowercase" | "uppercase" | "truncate" => {}
                other => {
                    bail!(
                        "Column '{}' mapping {} -> {} uses unsupported strategy '{}'",
                        column_name,
                        mapping.from,
                        mapping.to,
                        other
                    );
                }
            }
            if matches!(normalized.as_str(), "trim" | "lowercase" | "uppercase") {
                ensure!(
                    mapping.from == ColumnType::String && mapping.to == ColumnType::String,
                    "Column '{}' mapping {} -> {} cannot apply '{}' strategy",
                    column_name,
                    mapping.from,
                    mapping.to,
                    strategy
                );
            }
            if normalized == "round" {
                ensure!(
                    matches!(
                        mapping.to,
                        ColumnType::Float
                            | ColumnType::Integer
                            | ColumnType::String
                            | ColumnType::Currency
                            | ColumnType::Decimal(_)
                    ),
                    "Column '{}' mapping {} -> {} cannot apply 'round' strategy",
                    column_name,
                    mapping.from,
                    mapping.to
                );
            }
            if normalized == "truncate" {
                ensure!(
                    matches!(
                        mapping.to,
                        ColumnType::Integer | ColumnType::Currency | ColumnType::Decimal(_)
                    ),
                    "Column '{}' mapping {} -> {} cannot apply 'truncate' strategy",
                    column_name,
                    mapping.from,
                    mapping.to
                );
            }
        }
    }

    if let Some(scale) = mapping.options.get("scale") {
        let numeric = if let Some(value) = scale.as_u64() {
            value
        } else if let Some(value) = scale.as_i64() {
            ensure!(
                value >= 0,
                "Column '{}' mapping {} -> {} requires a non-negative scale",
                column_name,
                mapping.from,
                mapping.to
            );
            value as u64
        } else {
            bail!(
                "Column '{}' mapping {} -> {} requires 'scale' to be a number",
                column_name,
                mapping.from,
                mapping.to
            );
        };

        if mapping.to == ColumnType::Currency {
            ensure!(
                crate::data::CURRENCY_ALLOWED_SCALES.contains(&(numeric as u32)),
                "Column '{}' mapping {} -> {} requires scale to be 2 or 4",
                column_name,
                mapping.from,
                mapping.to
            );
        }
        if matches!(mapping.to, ColumnType::Decimal(_)) {
            bail!(
                "Column '{}' mapping {} -> {} should define scale via the decimal datatype rather than a mapping option",
                column_name,
                mapping.from,
                mapping.to
            );
        }
    }

    if let Some(format_value) = mapping.options.get("format") {
        ensure!(
            format_value.as_str().is_some(),
            "Column '{}' mapping {} -> {} requires 'format' to be a string",
            column_name,
            mapping.from,
            mapping.to
        );
    }

    if mapping.options.contains_key("precision") {
        bail!(
            "Column '{}' mapping {} -> {} should define precision via the decimal datatype rather than a mapping option",
            column_name,
            mapping.from,
            mapping.to
        );
    }

    Ok(())
}

#[derive(Debug, Clone)]
struct TypeCandidate {
    non_empty: usize,
    boolean_matches: usize,
    integer_matches: usize,
    integer_max_digits: u32,
    float_matches: usize,
    decimal_matches: usize,
    decimal_max_precision: u32,
    decimal_max_scale: u32,
    decimal_max_integer_digits: u32,
    decimal_precision_overflow: bool,
    date_matches: usize,
    datetime_matches: usize,
    time_matches: usize,
    guid_matches: usize,
    currency_matches: usize,
    currency_symbol_hits: usize,
    unclassified: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[doc(hidden)]
pub enum NumericKind {
    Integer,
    Decimal,
    Float,
}

#[derive(Debug, Clone, Copy)]
#[doc(hidden)]
pub struct NumericObservation {
    pub kind: NumericKind,
    pub precision: u32,
    pub scale: u32,
    pub integer_digits: u32,
    pub has_currency_symbol: bool,
    pub fits_currency_scale: bool,
    pub overflow: bool,
}

impl NumericObservation {
    fn integer(integer_digits: u32, has_currency_symbol: bool) -> Self {
        Self {
            kind: NumericKind::Integer,
            precision: integer_digits,
            scale: 0,
            integer_digits,
            has_currency_symbol,
            fits_currency_scale: true,
            overflow: false,
        }
    }

    fn decimal(
        precision: u32,
        scale: u32,
        integer_digits: u32,
        has_currency_symbol: bool,
        fits_currency_scale: bool,
        overflow: bool,
    ) -> Self {
        Self {
            kind: NumericKind::Decimal,
            precision,
            scale,
            integer_digits,
            has_currency_symbol,
            fits_currency_scale,
            overflow,
        }
    }

    fn float(has_currency_symbol: bool) -> Self {
        Self {
            kind: NumericKind::Float,
            precision: 0,
            scale: 0,
            integer_digits: 0,
            has_currency_symbol,
            fits_currency_scale: false,
            overflow: false,
        }
    }
}

#[doc(hidden)]
pub fn analyze_numeric_token(value: &str) -> Option<NumericObservation> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }

    let mut body = trimmed;
    let mut had_parentheses = false;
    if body.starts_with('(') && body.ends_with(')') && body.len() > 2 {
        had_parentheses = true;
        body = &body[1..body.len() - 1];
    }

    body = body.trim();
    if body.is_empty() {
        return None;
    }

    let mut mantissa = String::with_capacity(body.len());
    let mut exponent = String::new();
    let mut in_exponent = false;
    let mut exponent_sign_allowed = false;
    let mut decimal_index: Option<usize> = None;
    let mut has_currency_symbol = false;
    let mut sign_consumed = had_parentheses;

    for ch in body.chars() {
        match ch {
            '0'..='9' => {
                if in_exponent {
                    exponent.push(ch);
                } else {
                    mantissa.push(ch);
                }
            }
            '.' => {
                if in_exponent || decimal_index.is_some() {
                    return None;
                }
                decimal_index = Some(mantissa.len());
            }
            'e' | 'E' => {
                if in_exponent {
                    return None;
                }
                in_exponent = true;
                exponent_sign_allowed = true;
                continue;
            }
            '+' | '-' => {
                if in_exponent && exponent_sign_allowed {
                    exponent.push(ch);
                    exponent_sign_allowed = false;
                } else if !in_exponent && mantissa.is_empty() && !sign_consumed {
                    sign_consumed = true;
                } else {
                    return None;
                }
            }
            ',' | '_' | ' ' => {
                continue;
            }
            '$' | '€' | '£' | '¥' => {
                has_currency_symbol = true;
                continue;
            }
            _ => {
                return None;
            }
        }
        if ch != '+' && ch != '-' {
            exponent_sign_allowed = false;
        }
    }

    if mantissa.is_empty() {
        return None;
    }

    if decimal_index.is_none()
        && !in_exponent
        && mantissa.len() > 1
        && mantissa.chars().all(|c| c == '0')
    {
        return None;
    }
    if decimal_index.is_none() && !in_exponent && mantissa.len() > 1 && mantissa.starts_with('0') {
        return None;
    }

    let mantissa_scale = decimal_index.map(|pos| mantissa.len() - pos).unwrap_or(0);

    let exponent_value = if in_exponent {
        if exponent.is_empty() || exponent == "+" || exponent == "-" {
            return None;
        }
        match exponent.parse::<i32>() {
            Ok(value) => value,
            Err(_) => return None,
        }
    } else {
        0
    };

    let mut digits = mantissa.clone();
    let mut scale_i32 = mantissa_scale as i32 - exponent_value;
    if scale_i32 < 0 {
        let zeros = (-scale_i32) as usize;
        digits.push_str(&"0".repeat(zeros));
        scale_i32 = 0;
    }
    let scale = scale_i32.max(0) as u32;
    let digits_len = digits.len() as u32;
    let integer_digits = digits_len.saturating_sub(scale);

    let mut precision = if digits_len == 0 {
        0
    } else if integer_digits == 0 {
        scale.max(1)
    } else {
        integer_digits + scale
    };
    if precision == 0 {
        precision = 1;
    }

    let fits_currency_scale = scale == 0 || crate::data::CURRENCY_ALLOWED_SCALES.contains(&scale);
    let overflow = precision > DECIMAL_MAX_PRECISION || scale > DECIMAL_MAX_PRECISION;

    if in_exponent || decimal_index.is_some() || scale > 0 {
        return Some(NumericObservation::decimal(
            precision,
            scale,
            integer_digits,
            has_currency_symbol || had_parentheses,
            fits_currency_scale,
            overflow,
        ));
    }

    if overflow {
        return Some(NumericObservation::float(
            has_currency_symbol || had_parentheses,
        ));
    }

    Some(NumericObservation::integer(
        integer_digits,
        has_currency_symbol || had_parentheses,
    ))
}

const CURRENCY_SYMBOL_PROMOTION_THRESHOLD: usize = 30;
const SUMMARY_TRACKED_LIMIT: usize = 5;
const CURRENT_SCHEMA_VERSION: &str = "1.1.0";

#[derive(Clone, Default)]
struct SummaryAccumulator {
    non_empty: usize,
    tracked: Vec<(String, usize)>,
    other_values: usize,
}

impl SummaryAccumulator {
    fn record(&mut self, value: &str) {
        self.non_empty += 1;
        if let Some((_, count)) = self
            .tracked
            .iter_mut()
            .find(|(existing, _)| existing == value)
        {
            *count += 1;
            return;
        }
        if self.tracked.len() < SUMMARY_TRACKED_LIMIT {
            self.tracked.push((value.to_string(), 1));
        } else {
            self.other_values += 1;
        }
    }

    fn finalize(self) -> ColumnSummary {
        ColumnSummary {
            non_empty: self.non_empty,
            tracked_values: self.tracked,
            other_values: self.other_values,
        }
    }
}

impl TypeCandidate {
    fn new() -> Self {
        Self {
            non_empty: 0,
            boolean_matches: 0,
            integer_matches: 0,
            integer_max_digits: 0,
            float_matches: 0,
            decimal_matches: 0,
            decimal_max_precision: 0,
            decimal_max_scale: 0,
            decimal_max_integer_digits: 0,
            decimal_precision_overflow: false,
            date_matches: 0,
            datetime_matches: 0,
            time_matches: 0,
            guid_matches: 0,
            currency_matches: 0,
            currency_symbol_hits: 0,
            unclassified: 0,
        }
    }

    fn update(&mut self, value: &str) {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return;
        }

        let lowered = trimmed.to_ascii_lowercase();
        if is_placeholder_token(&lowered) {
            return;
        }

        self.non_empty += 1;
        let mut parsed_any = false;

        if matches!(
            lowered.as_str(),
            "true" | "false" | "t" | "f" | "yes" | "no" | "y" | "n"
        ) {
            self.boolean_matches += 1;
            parsed_any = true;
        }

        if let Some(observation) = analyze_numeric_token(trimmed) {
            parsed_any = true;
            match observation.kind {
                NumericKind::Integer => {
                    self.integer_matches += 1;
                    self.integer_max_digits =
                        self.integer_max_digits.max(observation.integer_digits);
                    if observation.fits_currency_scale {
                        self.currency_matches += 1;
                    }
                }
                NumericKind::Decimal => {
                    self.decimal_matches += 1;
                    self.decimal_max_precision =
                        self.decimal_max_precision.max(observation.precision);
                    self.decimal_max_scale = self.decimal_max_scale.max(observation.scale);
                    self.decimal_max_integer_digits = self
                        .decimal_max_integer_digits
                        .max(observation.integer_digits);
                    if observation.fits_currency_scale {
                        self.currency_matches += 1;
                    }
                    if observation.overflow {
                        self.decimal_precision_overflow = true;
                        self.float_matches += 1;
                    }
                }
                NumericKind::Float => {
                    self.float_matches += 1;
                }
            }
            if observation.has_currency_symbol {
                self.currency_symbol_hits += 1;
            }
        }

        if !parsed_any && parse_naive_date(trimmed).is_ok() {
            self.date_matches += 1;
            parsed_any = true;
        }
        if !parsed_any && parse_naive_datetime(trimmed).is_ok() {
            self.datetime_matches += 1;
            parsed_any = true;
        }
        if !parsed_any && parse_naive_time(trimmed).is_ok() {
            self.time_matches += 1;
            parsed_any = true;
        }

        let trimmed_guid = trimmed.trim_matches(|c| matches!(c, '{' | '}'));
        if !parsed_any && Uuid::parse_str(trimmed_guid).is_ok() {
            self.guid_matches += 1;
            parsed_any = true;
        }

        if !parsed_any {
            self.unclassified += 1;
        }
    }

    fn majority(&self, count: usize) -> bool {
        count > 0 && count * 2 > self.non_empty
    }

    fn decimal_spec(&self) -> Option<DecimalSpec> {
        if self.decimal_matches == 0 {
            return None;
        }
        if self.decimal_precision_overflow {
            return None;
        }

        let scale = self.decimal_max_scale.min(DECIMAL_MAX_PRECISION);
        let integer_digits = self.decimal_max_integer_digits.max(self.integer_max_digits);

        let mut precision = if integer_digits == 0 {
            scale.max(1)
        } else {
            integer_digits + scale
        };
        precision = precision.max(self.decimal_max_precision);

        if precision > DECIMAL_MAX_PRECISION {
            return None;
        }

        DecimalSpec::new(precision, scale).ok()
    }

    fn decide(&self) -> ColumnType {
        if self.non_empty == 0 {
            return ColumnType::String;
        }
        if self.unclassified > 0 {
            return ColumnType::String;
        }
        let promote_currency = self.should_promote_currency();
        if self.majority(self.boolean_matches) {
            ColumnType::Boolean
        } else if promote_currency {
            ColumnType::Currency
        } else if let Some(spec) = self.decimal_spec() {
            ColumnType::Decimal(spec)
        } else if self.decimal_matches > 0 {
            ColumnType::Float
        } else if self.majority(self.integer_matches) {
            ColumnType::Integer
        } else if self.majority(self.currency_matches) && self.currency_symbol_hits > 0 {
            ColumnType::Currency
        } else if self.majority(self.float_matches) {
            ColumnType::Float
        } else if self.majority(self.date_matches) {
            ColumnType::Date
        } else if self.majority(self.datetime_matches) {
            ColumnType::DateTime
        } else if self.majority(self.time_matches) {
            ColumnType::Time
        } else if self.majority(self.guid_matches) {
            ColumnType::Guid
        } else {
            ColumnType::String
        }
    }

    fn currency_symbol_ratio_meets_threshold(&self) -> bool {
        if self.non_empty == 0 {
            return false;
        }
        self.currency_symbol_hits.saturating_mul(100)
            >= self
                .non_empty
                .saturating_mul(CURRENCY_SYMBOL_PROMOTION_THRESHOLD)
    }

    fn should_promote_currency(&self) -> bool {
        self.currency_matches > 0
            && self.currency_matches == self.non_empty
            && self.currency_symbol_ratio_meets_threshold()
    }
}

fn is_placeholder_token(lowered: &str) -> bool {
    let stripped = lowered.trim_start_matches('#');
    matches!(
        stripped,
        "na" | "n/a" | "n.a." | "null" | "none" | "unknown" | "missing"
    ) || stripped.starts_with("invalid")
        || stripped.chars().all(|c| c == '-')
}

fn placeholder_token_original(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    let lowered = trimmed.to_ascii_lowercase();
    if is_placeholder_token(&lowered) {
        Some(trimmed.to_string())
    } else {
        None
    }
}

fn build_header_aliases(header: &str) -> HashSet<String> {
    let mut aliases = HashSet::new();
    let trimmed = header.trim();
    if trimmed.is_empty() {
        return aliases;
    }

    let mut try_insert = |candidate: &str| {
        let token = candidate.trim();
        if token.is_empty() {
            return;
        }
        aliases.insert(token.to_ascii_lowercase());
    };

    try_insert(trimmed);

    for sep in ['_', ' ', '/'] {
        if let Some(part) = trimmed.rsplit(sep).next()
            && part != trimmed
        {
            try_insert(part);
        }
    }

    let sanitized: String = trimmed
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '+' | '-'))
        .collect();
    if !sanitized.is_empty() {
        try_insert(&sanitized);
        if sanitized.len() >= 2 {
            let chars: Vec<char> = sanitized.chars().collect();
            let first = chars.first().copied().unwrap();
            let last = chars.last().copied().unwrap_or(first);
            let shorthand = format!("{}{}", first, last);
            try_insert(&shorthand);
        }
        if sanitized.len() >= 3 {
            try_insert(&sanitized[..3]);
        }
        if sanitized.len() >= 4 {
            try_insert(&sanitized[..4]);
        }
    }

    aliases
}

fn row_values_look_like_header<'a, I>(row: I, header_aliases: &[HashSet<String>]) -> bool
where
    I: IntoIterator<Item = Option<Cow<'a, str>>>,
{
    let mut alias_hits = 0usize;
    let mut non_empty_fields = 0usize;

    for (idx, value_opt) in row.into_iter().enumerate() {
        if idx >= header_aliases.len() {
            break;
        }
        let Some(value) = value_opt else {
            continue;
        };
        let trimmed = value.trim();
        if trimmed.is_empty() {
            continue;
        }
        non_empty_fields += 1;
        let lowered = trimmed.to_ascii_lowercase();
        if header_aliases[idx].contains(&lowered) {
            alias_hits += 1;
        }
    }

    non_empty_fields >= HEADER_ALIAS_MIN_MATCHES
        && alias_hits >= HEADER_ALIAS_MIN_MATCHES
        && alias_hits.saturating_mul(100)
            >= non_empty_fields.saturating_mul(HEADER_ALIAS_THRESHOLD_PERCENT)
}

fn option_row_looks_like_header(
    row: &[Option<String>],
    header_aliases: &[HashSet<String>],
) -> bool {
    row_values_look_like_header(
        row.iter().map(|value| value.as_deref().map(Cow::Borrowed)),
        header_aliases,
    )
}

pub(crate) fn row_looks_like_header(row: &[String], header_aliases: &[HashSet<String>]) -> bool {
    row_values_look_like_header(
        row.iter().map(|value| Some(Cow::Borrowed(value.as_str()))),
        header_aliases,
    )
}

fn generate_field_names(count: usize) -> Vec<String> {
    (0..count).map(|idx| format!("field_{idx}")).collect()
}

fn token_is_common_header(value: &str) -> bool {
    if value.is_empty() {
        return false;
    }
    let normalized = value.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        return false;
    }
    if COMMON_HEADER_TOKENS
        .iter()
        .any(|token| normalized == *token)
    {
        return true;
    }
    let sanitized = normalized
        .chars()
        .map(|ch| match ch {
            ' ' | '-' | '/' => '_',
            other => other,
        })
        .collect::<String>();
    COMMON_HEADER_TOKENS.iter().any(|token| sanitized == *token)
}

fn value_is_data_like(value: &str) -> bool {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return false;
    }
    let lowered = trimmed.to_ascii_lowercase();
    if matches!(
        lowered.as_str(),
        "true" | "false" | "t" | "f" | "yes" | "no" | "y" | "n" | "1" | "0"
    ) {
        return true;
    }
    if parse_decimal_literal(trimmed).is_ok() {
        return true;
    }
    if parse_currency_decimal(trimmed).is_ok() {
        return true;
    }
    if trimmed.parse::<i64>().is_ok() {
        return true;
    }
    if trimmed.parse::<f64>().is_ok() {
        return true;
    }
    if parse_naive_datetime(trimmed).is_ok() {
        return true;
    }
    if parse_naive_date(trimmed).is_ok() {
        return true;
    }
    if parse_naive_time(trimmed).is_ok() {
        return true;
    }
    let trimmed_guid = trimmed.trim_matches(|c| matches!(c, '{' | '}'));
    Uuid::parse_str(trimmed_guid).is_ok()
}

fn value_is_header_like(value: &str) -> bool {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return false;
    }
    if value_is_data_like(trimmed) {
        return false;
    }
    trimmed.chars().any(|c| c.is_ascii_alphabetic()) || token_is_common_header(trimmed)
}

fn header_tokens_match_dictionary(row: &[String]) -> bool {
    row.iter()
        .filter(|value| token_is_common_header(value.trim()))
        .count()
        >= 2
}

fn infer_has_header(first_row: &[String], other_rows: &[Vec<String>]) -> bool {
    let header_like_first = first_row
        .iter()
        .filter(|value| value_is_header_like(value))
        .count();
    let data_like_first = first_row
        .iter()
        .filter(|value| value_is_data_like(value))
        .count();

    if header_like_first == 0 && data_like_first == 0 {
        return false;
    }

    if data_like_first > header_like_first {
        return false;
    }

    if other_rows.is_empty() {
        return header_like_first >= 2 || header_tokens_match_dictionary(first_row);
    }

    let mut header_signal = 0usize;
    let mut data_signal = 0usize;

    for column in 0..first_row.len() {
        let first_value = first_row.get(column).map(|s| s.as_str()).unwrap_or("");
        let first_is_header = value_is_header_like(first_value);
        let first_is_data = value_is_data_like(first_value);

        let mut other_has_data = false;
        for row in other_rows {
            if let Some(value) = row.get(column)
                && value_is_data_like(value)
            {
                other_has_data = true;
                break;
            }
        }

        if first_is_header && other_has_data {
            header_signal += 1;
        } else if first_is_data && other_has_data {
            data_signal += 1;
        }
    }

    if header_signal > data_signal {
        return true;
    }
    if data_signal > header_signal {
        return false;
    }

    if header_tokens_match_dictionary(first_row) && header_like_first >= 1 {
        return true;
    }

    header_like_first > data_like_first
}

pub fn detect_csv_layout(
    path: &Path,
    delimiter: u8,
    encoding: &'static Encoding,
    header_override: Option<bool>,
) -> Result<CsvLayout> {
    if io_utils::is_dash(path) {
        return Ok(CsvLayout {
            headers: Vec::new(),
            has_headers: header_override.unwrap_or(true),
        });
    }

    if let Some(force_header) = header_override {
        let mut reader = io_utils::open_csv_reader_from_path(path, delimiter, force_header)?;
        if force_header {
            let header_record = reader.byte_headers()?.clone();
            let headers = io_utils::decode_headers(&header_record, encoding)?;
            return Ok(CsvLayout {
                headers,
                has_headers: true,
            });
        } else {
            let mut record = csv::ByteRecord::new();
            let width = if reader.read_byte_record(&mut record)? {
                record.len()
            } else {
                0
            };
            let headers = generate_field_names(width);
            return Ok(CsvLayout {
                headers,
                has_headers: false,
            });
        }
    }

    let mut reader = io_utils::open_csv_reader_from_path(path, delimiter, false)?;
    let mut record = csv::ByteRecord::new();
    let mut decoded_rows = Vec::new();

    while decoded_rows.len() < HEADER_DETECTION_SAMPLE_ROWS
        && reader.read_byte_record(&mut record)?
    {
        let decoded = io_utils::decode_record(&record, encoding)?;
        decoded_rows.push(decoded);
    }

    if decoded_rows.is_empty() {
        return Ok(CsvLayout {
            headers: Vec::new(),
            has_headers: true,
        });
    }

    let first_row = decoded_rows.first().cloned().unwrap_or_default();
    let has_header = infer_has_header(&first_row, &decoded_rows[1..]);
    let headers = if has_header {
        first_row
    } else {
        generate_field_names(first_row.len())
    };

    Ok(CsvLayout {
        headers,
        has_headers: has_header,
    })
}

pub fn infer_schema(
    path: &Path,
    sample_rows: usize,
    delimiter: u8,
    encoding: &'static Encoding,
    header_override: Option<bool>,
) -> Result<Schema> {
    let policy = PlaceholderPolicy::default();
    let (schema, _stats) = infer_schema_with_stats(
        path,
        sample_rows,
        delimiter,
        encoding,
        &policy,
        header_override,
    )?;
    Ok(schema)
}

pub fn infer_schema_with_stats(
    path: &Path,
    sample_rows: usize,
    delimiter: u8,
    encoding: &'static Encoding,
    _placeholder_policy: &PlaceholderPolicy,
    header_override: Option<bool>,
) -> Result<(Schema, InferenceStats)> {
    let layout = detect_csv_layout(path, delimiter, encoding, header_override)?;
    let mut reader = io_utils::open_csv_reader_from_path(path, delimiter, layout.has_headers)?;
    let headers = if layout.has_headers {
        let header_record = reader.byte_headers()?.clone();
        io_utils::decode_headers(&header_record, encoding)?
    } else {
        layout.headers.clone()
    };
    let mut candidates = vec![TypeCandidate::new(); headers.len()];
    let mut samples = vec![None; headers.len()];
    let mut summaries = vec![SummaryAccumulator::default(); headers.len()];
    let mut placeholders = vec![PlaceholderSummary::default(); headers.len()];
    let header_aliases: Vec<HashSet<String>> = headers
        .iter()
        .map(|header| build_header_aliases(header))
        .collect();

    let mut record = csv::ByteRecord::new();
    let mut processed = 0usize;
    let mut decode_errors = 0usize;
    while reader.read_byte_record(&mut record)? {
        if sample_rows > 0 && processed >= sample_rows {
            break;
        }
        let mut decoded_row: Vec<Option<String>> = Vec::with_capacity(headers.len());

        for field in record.iter().take(headers.len()) {
            if field.is_empty() {
                decoded_row.push(None);
                continue;
            }
            match io_utils::decode_bytes(field, encoding) {
                Ok(decoded) => {
                    let trimmed = decoded.trim();
                    if trimmed.is_empty() {
                        decoded_row.push(None);
                        continue;
                    }
                    let value = trimmed.to_string();
                    decoded_row.push(Some(value));
                }
                Err(_) => {
                    decode_errors += 1;
                    decoded_row.push(None);
                }
            }
        }

        while decoded_row.len() < headers.len() {
            decoded_row.push(None);
        }

        let header_like = option_row_looks_like_header(&decoded_row, &header_aliases);

        if header_like {
            continue;
        }

        for (idx, value_opt) in decoded_row.into_iter().enumerate() {
            let Some(value) = value_opt else {
                continue;
            };
            if let Some(token) = placeholder_token_original(&value) {
                placeholders[idx].record(&token);
                continue;
            }
            candidates[idx].update(&value);
            summaries[idx].record(&value);
            if samples[idx].is_none() {
                samples[idx] = Some(value.clone());
            }
        }
        processed += 1;
    }

    let columns = headers
        .iter()
        .enumerate()
        .map(|(idx, header)| ColumnMeta {
            name: header.clone(),
            datatype: candidates[idx].decide(),
            rename: None,
            value_replacements: Vec::new(),
            datatype_mappings: Vec::new(),
        })
        .collect();

    let schema = Schema {
        columns,
        schema_version: None,
        has_headers: layout.has_headers,
    };
    let stats = InferenceStats {
        sample_values: samples,
        rows_read: processed,
        requested_rows: sample_rows,
        decode_errors,
        summaries: summaries
            .into_iter()
            .map(SummaryAccumulator::finalize)
            .collect(),
        placeholder_summaries: placeholders,
    };

    Ok((schema, stats))
}

#[doc(hidden)]
pub fn format_hint_for(datatype: &ColumnType, sample: Option<&str>) -> Option<String> {
    let sample = sample?;
    match datatype {
        ColumnType::DateTime => {
            if sample.contains('T') {
                Some("ISO 8601 date-time".to_string())
            } else if sample.contains('/') {
                Some("Slash-separated date-time".to_string())
            } else if sample.contains('-') {
                Some("Hyphen-separated date-time".to_string())
            } else {
                Some("Date-time without delimiter hints".to_string())
            }
        }
        ColumnType::Date => {
            if sample.contains('/') {
                Some("Slash-separated date".to_string())
            } else if sample.contains('-') {
                Some("Hyphen-separated date".to_string())
            } else if sample.contains('.') {
                Some("Dot-separated date".to_string())
            } else {
                Some("Date without delimiter hints".to_string())
            }
        }
        ColumnType::Time => {
            if sample.contains('.') {
                Some("Time with fractional seconds".to_string())
            } else {
                Some("Colon-separated time".to_string())
            }
        }
        ColumnType::Boolean => {
            let normalized = sample.trim().to_ascii_lowercase();
            if matches!(normalized.as_str(), "true" | "false" | "t" | "f") {
                Some("Boolean (true/false tokens)".to_string())
            } else if matches!(normalized.as_str(), "yes" | "no" | "y" | "n") {
                Some("Boolean (yes/no tokens)".to_string())
            } else if matches!(normalized.as_str(), "1" | "0") {
                Some("Boolean (1/0 tokens)".to_string())
            } else {
                Some("Boolean (mixed tokens)".to_string())
            }
        }
        ColumnType::Float => {
            let has_currency = ["$", "€", "£", "¥"]
                .iter()
                .any(|symbol| sample.contains(symbol));
            if has_currency {
                Some("Currency symbol detected".to_string())
            } else if sample.contains(',') {
                Some("Thousands separator present".to_string())
            } else if sample.contains('.') {
                Some("Decimal point".to_string())
            } else {
                Some("Floating number without decimal point".to_string())
            }
        }
        ColumnType::Decimal(spec) => Some(format!(
            "Fixed decimal (precision {}, scale {})",
            spec.precision, spec.scale
        )),
        ColumnType::Currency => Some("Currency amount (2 or 4 decimal places)".to_string()),
        ColumnType::Integer => {
            if sample.starts_with('0') && sample.len() > 1 {
                Some("Leading zeros preserved".to_string())
            } else {
                Some("Whole number".to_string())
            }
        }
        ColumnType::Guid => {
            if sample.contains('{') || sample.contains('}') {
                Some("GUID with braces".to_string())
            } else if sample.contains('-') {
                Some("Canonical GUID".to_string())
            } else {
                Some("GUID without separators".to_string())
            }
        }
        ColumnType::String => None,
    }
}

impl ColumnMeta {
    pub fn has_mappings(&self) -> bool {
        !self.datatype_mappings.is_empty()
    }

    pub fn output_name(&self) -> &str {
        self.rename
            .as_deref()
            .filter(|value| !value.is_empty())
            .unwrap_or(&self.name)
    }

    pub fn matches_header(&self, header: &str) -> bool {
        if header == self.name {
            return true;
        }
        if let Some(rename) = self.rename.as_deref()
            && !rename.is_empty()
            && header == rename
        {
            return true;
        }
        false
    }

    pub fn apply_mappings_to_value(&self, value: &str) -> Result<Option<String>> {
        if value.is_empty() {
            return Ok(None);
        }
        if !self.has_mappings() {
            return Ok(Some(value.to_string()));
        }

        let first_mapping = self
            .datatype_mappings
            .first()
            .expect("has_mappings() guarantees at least one mapping");

        let mut current = parse_initial_value(value, first_mapping)?;
        for mapping in &self.datatype_mappings {
            let current_type = value_column_type(&current);
            ensure!(
                current_type == mapping.from,
                "Datatype mapping chain expects '{:?}' but encountered '{:?}'",
                mapping.from,
                current_type
            );
            current = apply_single_mapping(mapping, current)?;
        }

        let last_mapping = self
            .datatype_mappings
            .last()
            .expect("non-empty mapping chain");
        let rendered = render_mapped_value(&current, last_mapping)?;
        if rendered.is_empty() {
            Ok(None)
        } else {
            Ok(Some(rendered))
        }
    }

    pub fn normalize_value<'a>(&self, value: &'a str) -> Cow<'a, str> {
        for replacement in &self.value_replacements {
            if value == replacement.from {
                return Cow::Owned(replacement.to.clone());
            }
        }
        Cow::Borrowed(value)
    }
}

impl Schema {
    pub fn has_transformations(&self) -> bool {
        self.columns.iter().any(|column| column.has_mappings())
    }

    pub fn apply_transformations_to_row(&self, row: &mut [String]) -> Result<()> {
        for (idx, column) in self.columns.iter().enumerate() {
            if !column.has_mappings() {
                continue;
            }
            if let Some(cell) = row.get_mut(idx) {
                let original = cell.clone();
                match column
                    .apply_mappings_to_value(&original)
                    .with_context(|| format!("Column '{}'", column.name))?
                {
                    Some(mapped) => *cell = mapped,
                    None => cell.clear(),
                }
            }
        }
        Ok(())
    }

    pub fn apply_replacements_to_row(&self, row: &mut [String]) {
        for (idx, column) in self.columns.iter().enumerate() {
            if let Some(value) = row.get_mut(idx)
                && let Cow::Owned(normalized) = column.normalize_value(value)
            {
                *value = normalized;
            }
        }
    }

    pub fn validate_datatype_mappings(&self) -> Result<()> {
        self.validate_decimal_specs()?;
        for column in &self.columns {
            if column.datatype_mappings.is_empty() {
                continue;
            }
            let mut previous_to = None;
            for (step_index, mapping) in column.datatype_mappings.iter().enumerate() {
                if let Some(expected) = previous_to.as_ref() {
                    ensure!(
                        mapping.from == *expected,
                        "Column '{}' mapping step {} expects input '{:?}' but prior step outputs '{:?}'",
                        column.name,
                        step_index + 1,
                        mapping.from,
                        expected
                    );
                }
                validate_mapping_options(&column.name, mapping)?;
                previous_to = Some(mapping.to.clone());
            }
            let terminal = previous_to.expect("mapping chain must have terminal type");
            ensure!(
                terminal == column.datatype,
                "Column '{}' mappings terminate at '{:?}' but column datatype is '{:?}'",
                column.name,
                terminal,
                column.datatype
            );
        }
        Ok(())
    }

    fn validate_decimal_specs(&self) -> Result<()> {
        for column in &self.columns {
            if let ColumnType::Decimal(spec) = &column.datatype {
                spec.ensure_valid()?;
            }
            for mapping in &column.datatype_mappings {
                if let ColumnType::Decimal(spec) = &mapping.from {
                    spec.ensure_valid()?;
                }
                if let ColumnType::Decimal(spec) = &mapping.to {
                    spec.ensure_valid()?;
                }
            }
        }
        Ok(())
    }
}

pub fn apply_placeholder_replacements(
    schema: &mut Schema,
    stats: &InferenceStats,
    policy: &PlaceholderPolicy,
) -> usize {
    let replacement_value = match policy {
        PlaceholderPolicy::TreatAsEmpty => String::new(),
        PlaceholderPolicy::FillWith(value) => value.clone(),
    };
    let mut added = 0usize;
    for (idx, column) in schema.columns.iter_mut().enumerate() {
        let Some(summary) = stats.placeholder_summary(idx) else {
            continue;
        };
        let entries = summary.entries();
        if entries.is_empty() {
            continue;
        }
        for (token, _) in entries {
            if column
                .value_replacements
                .iter()
                .any(|existing| existing.from == token)
            {
                continue;
            }
            column.value_replacements.push(ValueReplacement {
                from: token,
                to: replacement_value.clone(),
            });
            added += 1;
        }
    }
    added
}

pub mod evolution {
    use super::{ColumnMeta, ColumnType, Schema, ValueReplacement};
    use serde::{Deserialize, Serialize};
    use std::collections::BTreeMap;

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    pub struct SchemaChange {
        pub column: String,
        pub change: SchemaChangeKind,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    #[serde(tag = "kind", rename_all = "snake_case")]
    pub enum SchemaChangeKind {
        ColumnAdded,
        ColumnRemoved,
        ColumnRenamed {
            from: String,
            to: String,
        },
        DatatypeChanged {
            from: ColumnType,
            to: ColumnType,
        },
        RenameMappingAdded {
            from: String,
            to: String,
        },
        ReplaceMappingAdded {
            from_value: String,
            to_value: String,
        },
    }

    #[derive(Debug, Clone, Serialize, Deserialize, Default)]
    pub struct SchemaEvolution {
        pub changes: Vec<SchemaChange>,
    }

    impl SchemaEvolution {
        pub fn diff(previous: &Schema, current: &Schema) -> Self {
            let mut remaining: BTreeMap<&str, &ColumnMeta> = current
                .columns
                .iter()
                .map(|column| (column.name.as_str(), column))
                .collect();
            let mut changes = Vec::new();
            let mut unmatched_previous = Vec::new();

            for prev_column in &previous.columns {
                if let Some(curr_column) = remaining.remove(prev_column.name.as_str()) {
                    collect_column_diffs(
                        prev_column,
                        curr_column,
                        curr_column.name.as_str(),
                        &mut changes,
                    );
                } else {
                    unmatched_previous.push(prev_column);
                }
            }

            let mut leftovers: Vec<(&str, &ColumnMeta)> = remaining.into_iter().collect();

            for prev_column in unmatched_previous {
                if let Some(idx) = leftovers.iter().position(|(_, candidate)| {
                    candidate
                        .rename
                        .as_deref()
                        .map(|alias| alias == prev_column.name)
                        .unwrap_or(false)
                }) {
                    let (curr_name, curr_column) = leftovers.remove(idx);
                    changes.push(SchemaChange {
                        column: curr_name.to_string(),
                        change: SchemaChangeKind::ColumnRenamed {
                            from: prev_column.name.clone(),
                            to: curr_name.to_string(),
                        },
                    });
                    collect_column_diffs(prev_column, curr_column, curr_name, &mut changes);
                } else {
                    changes.push(SchemaChange {
                        column: prev_column.name.clone(),
                        change: SchemaChangeKind::ColumnRemoved,
                    });
                }
            }

            for (column_name, _) in leftovers {
                changes.push(SchemaChange {
                    column: column_name.to_string(),
                    change: SchemaChangeKind::ColumnAdded,
                });
            }

            changes.sort_by(|left, right| {
                left.change
                    .order()
                    .cmp(&right.change.order())
                    .then_with(|| left.column.cmp(&right.column))
            });

            SchemaEvolution { changes }
        }
    }

    fn collect_column_diffs(
        previous: &ColumnMeta,
        current: &ColumnMeta,
        column_name: &str,
        changes: &mut Vec<SchemaChange>,
    ) {
        if previous.datatype != current.datatype {
            changes.push(SchemaChange {
                column: column_name.to_string(),
                change: SchemaChangeKind::DatatypeChanged {
                    from: previous.datatype.clone(),
                    to: current.datatype.clone(),
                },
            });
        }

        if previous.rename != current.rename {
            if let Some(new_name) = &current.rename {
                changes.push(SchemaChange {
                    column: column_name.to_string(),
                    change: SchemaChangeKind::RenameMappingAdded {
                        from: previous.name.clone(),
                        to: new_name.clone(),
                    },
                });
            }
        }

        for replacement in &current.value_replacements {
            if !contains_replacement(&previous.value_replacements, replacement) {
                changes.push(SchemaChange {
                    column: column_name.to_string(),
                    change: SchemaChangeKind::ReplaceMappingAdded {
                        from_value: replacement.from.clone(),
                        to_value: replacement.to.clone(),
                    },
                });
            }
        }
    }

    fn contains_replacement(haystack: &[ValueReplacement], needle: &ValueReplacement) -> bool {
        haystack.iter().any(|candidate| candidate == needle)
    }

    impl SchemaChangeKind {
        fn order(&self) -> u8 {
            match self {
                SchemaChangeKind::ColumnAdded => 0,
                SchemaChangeKind::ColumnRemoved => 1,
                SchemaChangeKind::ColumnRenamed { .. } => 2,
                SchemaChangeKind::DatatypeChanged { .. } => 3,
                SchemaChangeKind::RenameMappingAdded { .. } => 4,
                SchemaChangeKind::ReplaceMappingAdded { .. } => 5,
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::super::{ColumnMeta, ColumnType, Schema, ValueReplacement};
        use super::*;

        #[test]
        fn diff_detects_add_remove_and_datatype_change() {
            let previous = Schema {
                columns: vec![ColumnMeta {
                    name: "id".to_string(),
                    datatype: ColumnType::Integer,
                    rename: None,
                    value_replacements: Vec::new(),
                    datatype_mappings: Vec::new(),
                }],
                schema_version: None,
                has_headers: true,
            };
            let current = Schema {
                columns: vec![
                    ColumnMeta {
                        name: "id".to_string(),
                        datatype: ColumnType::String,
                        rename: None,
                        value_replacements: Vec::new(),
                        datatype_mappings: Vec::new(),
                    },
                    ColumnMeta {
                        name: "status".to_string(),
                        datatype: ColumnType::String,
                        rename: None,
                        value_replacements: Vec::new(),
                        datatype_mappings: Vec::new(),
                    },
                ],
                schema_version: None,
                has_headers: true,
            };

            let evolution = SchemaEvolution::diff(&previous, &current);
            assert!(evolution.changes.iter().any(|change| matches!(
                change,
                SchemaChange {
                    column,
                    change: SchemaChangeKind::DatatypeChanged { .. }
                } if column == "id"
            )));
            assert!(evolution.changes.iter().any(|change| matches!(
                change,
                SchemaChange {
                    column,
                    change: SchemaChangeKind::ColumnAdded
                } if column == "status"
            )));
        }

        #[test]
        fn diff_detects_value_replacement_additions() {
            let previous = Schema {
                columns: vec![ColumnMeta {
                    name: "status".to_string(),
                    datatype: ColumnType::String,
                    rename: None,
                    value_replacements: Vec::new(),
                    datatype_mappings: Vec::new(),
                }],
                schema_version: None,
                has_headers: true,
            };
            let current = Schema {
                columns: vec![ColumnMeta {
                    name: "status".to_string(),
                    datatype: ColumnType::String,
                    rename: None,
                    value_replacements: vec![ValueReplacement {
                        from: "pending".to_string(),
                        to: "awaiting".to_string(),
                    }],
                    datatype_mappings: Vec::new(),
                }],
                schema_version: None,
                has_headers: true,
            };

            let evolution = SchemaEvolution::diff(&previous, &current);
            assert!(evolution.changes.iter().any(|change| matches!(
                change,
                SchemaChange {
                    change: SchemaChangeKind::ReplaceMappingAdded { from_value, to_value },
                    ..
                } if from_value == "pending" && to_value == "awaiting"
            )));
        }
    }
}

// tests moved to tests/schema_internals.rs
