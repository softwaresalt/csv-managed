use std::{
    borrow::Cow, collections::BTreeMap, fmt, fs::File, io::BufReader, path::Path, str::FromStr,
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
};

const DECIMAL_MAX_PRECISION: u32 = 28;

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
        struct ColumnTypeVisitor;

        impl<'de> de::Visitor<'de> for ColumnTypeVisitor {
            type Value = ColumnType;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str(
                    "a column datatype token such as 'Integer', 'String', or 'decimal(18,4)'",
                )
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                ColumnType::from_str(value).map_err(|err| E::custom(err.to_string()))
            }

            fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                self.visit_str(&value)
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: de::MapAccess<'de>,
            {
                use serde_yaml::Value as YamlValue;

                if let Some((key, value)) = map.next_entry::<String, YamlValue>()? {
                    let key_normalized = key.trim().to_ascii_lowercase();
                    match key_normalized.as_str() {
                        "decimal" => parse_decimal_from_mapping(value).map_err(de::Error::custom),
                        other => Err(de::Error::custom(format!(
                            "Unsupported structured datatype '{other}'"
                        ))),
                    }
                } else {
                    Err(de::Error::custom("Expected a column datatype entry"))
                }
            }
        }

        fn parse_decimal_from_mapping(value: serde_yaml::Value) -> Result<ColumnType> {
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
                        let parsed = val.as_u64().ok_or_else(|| {
                            anyhow!("Decimal precision must be an unsigned integer")
                        })?;
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

            let precision =
                precision.ok_or_else(|| anyhow!("Decimal mapping requires precision"))?;
            let scale = scale.ok_or_else(|| anyhow!("Decimal mapping requires scale"))?;
            let spec = DecimalSpec::new(precision, scale)?;
            Ok(ColumnType::Decimal(spec))
        }

        if deserializer.is_human_readable() {
            deserializer.deserialize_any(ColumnTypeVisitor)
        } else {
            deserializer.deserialize_str(ColumnTypeVisitor)
        }
    }
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

impl std::str::FromStr for ColumnType {
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
        }
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

    pub fn validate_headers(&self, headers: &[String]) -> Result<()> {
        if headers.len() != self.columns.len() {
            return Err(anyhow!(
                "Header length mismatch: schema expects {} column(s) but file contains {}",
                self.columns.len(),
                headers.len()
            ));
        }
        for (idx, column) in self.columns.iter().enumerate() {
            let name = headers.get(idx).map(|s| s.as_str()).unwrap_or_default();
            if name != column.name {
                return Err(anyhow!(
                    "Header mismatch at position {}: expected '{}' but found '{}'",
                    idx + 1,
                    column.name,
                    name
                ));
            }
        }
        Ok(())
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        self.save_internal(path, false)
    }

    pub fn save_with_replace_template(&self, path: &Path) -> Result<()> {
        self.save_internal(path, true)
    }

    pub fn load(path: &Path) -> Result<Self> {
        let file = File::open(path).with_context(|| format!("Opening schema file {path:?}"))?;
        let reader = BufReader::new(file);
        let schema: Schema = serde_yaml::from_reader(reader).context("Parsing schema YAML")?;
        schema.validate_datatype_mappings()?;
        Ok(schema)
    }

    fn save_internal(&self, path: &Path, include_replace_template: bool) -> Result<()> {
        let mut schema = self.clone();
        if schema.schema_version.is_none() {
            schema.schema_version = Some(CURRENT_SCHEMA_VERSION.to_string());
        }
        schema.validate_datatype_mappings()?;

        let file = File::create(path).with_context(|| format!("Creating schema file {path:?}"))?;
        if !include_replace_template {
            serde_yaml::to_writer(file, &schema).context("Writing schema YAML")
        } else {
            let mut value =
                serde_yaml::to_value(&schema).context("Serializing schema to YAML value")?;
            if let Some(columns) = value
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
            serde_yaml::to_writer(file, &value).context("Writing schema YAML")
        }
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
    float_matches: usize,
    date_matches: usize,
    datetime_matches: usize,
    time_matches: usize,
    guid_matches: usize,
    currency_matches: usize,
    currency_symbol_hits: usize,
    unclassified: usize,
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
            float_matches: 0,
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

        if trimmed.parse::<i64>().is_ok() {
            self.integer_matches += 1;
            parsed_any = true;
        }

        if trimmed.parse::<f64>().is_ok() {
            self.float_matches += 1;
            parsed_any = true;
        }

        if let Ok(decimal) = parse_currency_decimal(trimmed) {
            let scale = decimal.scale();
            let has_valid_scale =
                scale == 0 || crate::data::CURRENCY_ALLOWED_SCALES.contains(&scale);
            if has_valid_scale {
                self.currency_matches += 1;
                parsed_any = true;
                let has_symbol = trimmed.contains('$')
                    || trimmed.contains('€')
                    || trimmed.contains('£')
                    || trimmed.contains('¥');
                if has_symbol {
                    self.currency_symbol_hits += 1;
                }
            }
        }

        if parse_naive_date(trimmed).is_ok() {
            self.date_matches += 1;
            parsed_any = true;
        }
        if parse_naive_datetime(trimmed).is_ok() {
            self.datetime_matches += 1;
            parsed_any = true;
        }
        if parse_naive_time(trimmed).is_ok() {
            self.time_matches += 1;
            parsed_any = true;
        }

        let trimmed_guid = trimmed.trim_matches(|c| matches!(c, '{' | '}'));
        if Uuid::parse_str(trimmed_guid).is_ok() {
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

pub fn infer_schema(
    path: &Path,
    sample_rows: usize,
    delimiter: u8,
    encoding: &'static Encoding,
) -> Result<Schema> {
    let policy = PlaceholderPolicy::default();
    let (schema, _stats) =
        infer_schema_with_stats(path, sample_rows, delimiter, encoding, &policy)?;
    Ok(schema)
}

pub fn infer_schema_with_stats(
    path: &Path,
    sample_rows: usize,
    delimiter: u8,
    encoding: &'static Encoding,
    _placeholder_policy: &PlaceholderPolicy,
) -> Result<(Schema, InferenceStats)> {
    let mut reader = io_utils::open_csv_reader_from_path(path, delimiter, true)?;
    let header_record = reader.byte_headers()?.clone();
    let headers = io_utils::decode_headers(&header_record, encoding)?;
    let mut candidates = vec![TypeCandidate::new(); headers.len()];
    let mut samples = vec![None; headers.len()];
    let mut summaries = vec![SummaryAccumulator::default(); headers.len()];
    let mut placeholders = vec![PlaceholderSummary::default(); headers.len()];

    let mut record = csv::ByteRecord::new();
    let mut processed = 0usize;
    let mut decode_errors = 0usize;
    while reader.read_byte_record(&mut record)? {
        if sample_rows > 0 && processed >= sample_rows {
            break;
        }
        for (idx, field) in record.iter().enumerate() {
            if field.is_empty() {
                continue;
            }
            match io_utils::decode_bytes(field, encoding) {
                Ok(decoded) => {
                    if decoded.is_empty() {
                        continue;
                    }
                    if let Some(token) = placeholder_token_original(&decoded) {
                        placeholders[idx].record(&token);
                        continue;
                    }
                    candidates[idx].update(&decoded);
                    summaries[idx].record(&decoded);
                    if samples[idx].is_none() {
                        samples[idx] = Some(decoded.clone());
                    }
                }
                Err(_) => {
                    decode_errors += 1;
                }
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

pub(crate) fn format_hint_for(datatype: &ColumnType, sample: Option<&str>) -> Option<String> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use encoding_rs::UTF_8;
    use std::io::Write;
    use std::str::FromStr;
    use tempfile::NamedTempFile;

    #[test]
    fn infer_schema_with_stats_captures_samples() {
        let mut file = NamedTempFile::new().expect("temp file");
        writeln!(file, "id,date,value").unwrap();
        writeln!(file, "1,2024-01-01T08:30:00Z,$12.34").unwrap();
        writeln!(file, "2,2024-01-02T09:45:00Z,$56.78").unwrap();

        let policy = PlaceholderPolicy::default();
        let (schema, stats) = infer_schema_with_stats(file.path(), 0, b',', UTF_8, &policy)
            .expect("infer with stats");

        assert_eq!(schema.columns.len(), 3);
        assert_eq!(stats.sample_value(1), Some("2024-01-01T08:30:00Z"));
        assert_eq!(stats.sample_value(2), Some("$12.34"));
        assert_eq!(stats.rows_read(), 2);
        assert_eq!(stats.decode_errors(), 0);
    }

    #[test]
    fn format_hint_detects_common_patterns() {
        let date_hint = format_hint_for(&ColumnType::Date, Some("2024/01/30"));
        assert_eq!(date_hint.as_deref(), Some("Slash-separated date"));

        let currency_hint = format_hint_for(&ColumnType::Float, Some("€1,234.50"));
        assert_eq!(currency_hint.as_deref(), Some("Currency symbol detected"));

        let guid_hint = format_hint_for(
            &ColumnType::Guid,
            Some("{ABCDEF12-3456-7890-ABCD-EF1234567890}"),
        );
        assert_eq!(guid_hint.as_deref(), Some("GUID with braces"));
    }

    #[test]
    fn datatype_mappings_convert_string_to_date() {
        let mappings = vec![
            DatatypeMapping {
                from: ColumnType::String,
                to: ColumnType::DateTime,
                strategy: None,
                options: BTreeMap::new(),
            },
            DatatypeMapping {
                from: ColumnType::DateTime,
                to: ColumnType::Date,
                strategy: None,
                options: BTreeMap::new(),
            },
        ];

        let column = ColumnMeta {
            name: "event_date".to_string(),
            datatype: ColumnType::Date,
            rename: None,
            value_replacements: Vec::new(),
            datatype_mappings: mappings,
        };
        let schema = Schema {
            columns: vec![column],
            schema_version: None,
        };

        let mut row = vec!["2024-05-10T13:45:00".to_string()];
        schema
            .apply_transformations_to_row(&mut row)
            .expect("apply datatype mappings");
        assert_eq!(row[0], "2024-05-10");
    }

    #[test]
    fn datatype_mappings_round_float_values() {
        let mut options = BTreeMap::new();
        options.insert("scale".to_string(), Value::from(4));
        let mapping = DatatypeMapping {
            from: ColumnType::String,
            to: ColumnType::Float,
            strategy: Some("round".to_string()),
            options,
        };
        let column = ColumnMeta {
            name: "measurement".to_string(),
            datatype: ColumnType::Float,
            rename: None,
            value_replacements: Vec::new(),
            datatype_mappings: vec![mapping],
        };
        let schema = Schema {
            columns: vec![column],
            schema_version: None,
        };
        let mut row = vec!["3.1415926535".to_string()];
        schema
            .apply_transformations_to_row(&mut row)
            .expect("round float");
        assert_eq!(row[0], "3.1416");
    }

    #[test]
    fn datatype_mappings_round_currency_values() {
        let mut options = BTreeMap::new();
        options.insert("scale".to_string(), Value::from(2));
        let mapping = DatatypeMapping {
            from: ColumnType::String,
            to: ColumnType::Currency,
            strategy: Some("round".to_string()),
            options,
        };
        let column = ColumnMeta {
            name: "price".to_string(),
            datatype: ColumnType::Currency,
            rename: None,
            value_replacements: Vec::new(),
            datatype_mappings: vec![mapping],
        };
        let schema = Schema {
            columns: vec![column],
            schema_version: None,
        };
        let mut row = vec!["12.345".to_string()];
        schema
            .apply_transformations_to_row(&mut row)
            .expect("round currency");
        assert_eq!(row[0], "12.35");
    }

    #[test]
    fn datatype_mappings_preserve_currency_scale_when_unspecified() {
        let mapping = DatatypeMapping {
            from: ColumnType::String,
            to: ColumnType::Currency,
            strategy: None,
            options: BTreeMap::new(),
        };
        let column = ColumnMeta {
            name: "premium".to_string(),
            datatype: ColumnType::Currency,
            rename: None,
            value_replacements: Vec::new(),
            datatype_mappings: vec![mapping],
        };
        let schema = Schema {
            columns: vec![column],
            schema_version: None,
        };
        let mut row = vec!["123.4567".to_string()];
        schema
            .apply_transformations_to_row(&mut row)
            .expect("preserve currency scale");
        assert_eq!(row[0], "123.4567");
    }

    #[test]
    fn infer_schema_identifies_currency_columns() {
        let mut file = NamedTempFile::new().expect("temp file");
        writeln!(file, "amount,name").unwrap();
        writeln!(file, "$12.34,alpha").unwrap();
        writeln!(file, "56.7800,beta").unwrap();

        let policy = PlaceholderPolicy::default();
        let (schema, _) =
            infer_schema_with_stats(file.path(), 0, b',', UTF_8, &policy).expect("infer schema");
        assert_eq!(schema.columns.len(), 2);
        assert_eq!(schema.columns[0].datatype, ColumnType::Currency);
        assert_eq!(schema.columns[1].datatype, ColumnType::String);
    }

    #[test]
    fn infer_schema_promotes_currency_when_symbol_ratio_met() {
        let mut file = NamedTempFile::new().expect("temp file");
        writeln!(file, "amount").unwrap();
        writeln!(file, "$12.00").unwrap();
        writeln!(file, "14").unwrap();
        writeln!(file, "15").unwrap();

        let policy = PlaceholderPolicy::default();
        let (schema, _) =
            infer_schema_with_stats(file.path(), 0, b',', UTF_8, &policy).expect("infer schema");
        assert_eq!(schema.columns.len(), 1);
        assert_eq!(schema.columns[0].datatype, ColumnType::Currency);
    }

    #[test]
    fn infer_schema_prefers_majority_integer() {
        let mut file = NamedTempFile::new().expect("temp file");
        writeln!(file, "id,name").unwrap();
        writeln!(file, "1,alpha").unwrap();
        writeln!(file, "2,beta").unwrap();
        writeln!(file, "unknown,gamma").unwrap();

        let policy = PlaceholderPolicy::default();
        let (schema, _) =
            infer_schema_with_stats(file.path(), 0, b',', UTF_8, &policy).expect("infer schema");
        assert_eq!(schema.columns[0].datatype, ColumnType::Integer);
        assert_eq!(schema.columns[1].datatype, ColumnType::String);
    }

    #[test]
    fn infer_schema_prefers_majority_boolean() {
        let mut file = NamedTempFile::new().expect("temp file");
        writeln!(file, "flag").unwrap();
        writeln!(file, "true").unwrap();
        writeln!(file, "false").unwrap();
        writeln!(file, "unknown").unwrap();

        let policy = PlaceholderPolicy::default();
        let (schema, _) =
            infer_schema_with_stats(file.path(), 0, b',', UTF_8, &policy).expect("infer schema");
        assert_eq!(schema.columns.len(), 1);
        assert_eq!(schema.columns[0].datatype, ColumnType::Boolean);
    }

    #[test]
    fn infer_schema_collects_na_placeholders() {
        let mut file = NamedTempFile::new().expect("temp file");
        writeln!(file, "value").unwrap();
        writeln!(file, "NA").unwrap();
        writeln!(file, "#N/A").unwrap();
        writeln!(file, "42").unwrap();

        let policy = PlaceholderPolicy::default();
        let (_, stats) =
            infer_schema_with_stats(file.path(), 0, b',', UTF_8, &policy).expect("infer stats");

        let summary = stats.placeholder_summary(0).expect("placeholder summary");
        let entries = summary.entries();
        assert_eq!(entries.len(), 2);
        assert!(
            entries
                .iter()
                .any(|(token, count)| token == "NA" && *count == 1)
        );
        assert!(
            entries
                .iter()
                .any(|(token, count)| token == "#N/A" && *count == 1)
        );
    }

    #[test]
    fn apply_placeholder_replacements_respects_policy() {
        let mut file = NamedTempFile::new().expect("temp file");
        writeln!(file, "value").unwrap();
        writeln!(file, "NA").unwrap();
        writeln!(file, "#NA").unwrap();
        writeln!(file, "7").unwrap();

        let policy = PlaceholderPolicy::default();
        let (schema, stats) =
            infer_schema_with_stats(file.path(), 0, b',', UTF_8, &policy).expect("infer schema");

        let mut schema_empty = schema.clone();
        let added_empty = apply_placeholder_replacements(&mut schema_empty, &stats, &policy);
        assert_eq!(added_empty, 2);
        assert!(
            schema_empty.columns[0]
                .value_replacements
                .iter()
                .any(|r| r.from == "NA" && r.to.is_empty())
        );
        assert!(
            schema_empty.columns[0]
                .value_replacements
                .iter()
                .any(|r| r.from == "#NA" && r.to.is_empty())
        );

        let mut schema_fill = schema.clone();
        let fill_policy = PlaceholderPolicy::FillWith("NULL".to_string());
        let added_fill = apply_placeholder_replacements(&mut schema_fill, &stats, &fill_policy);
        assert_eq!(added_fill, 2);
        assert!(
            schema_fill.columns[0]
                .value_replacements
                .iter()
                .all(|r| r.to == "NULL")
        );

        let added_duplicate =
            apply_placeholder_replacements(&mut schema_fill, &stats, &fill_policy);
        assert_eq!(added_duplicate, 0);
    }

    #[test]
    fn parse_decimal_type_supports_positional_syntax() {
        let parsed = ColumnType::from_str("decimal(18,4)").expect("parse decimal positional");
        match parsed {
            ColumnType::Decimal(spec) => {
                assert_eq!(spec.precision, 18);
                assert_eq!(spec.scale, 4);
            }
            other => panic!("expected decimal column, got {other:?}"),
        }
    }

    #[test]
    fn parse_decimal_type_supports_named_syntax() {
        let parsed =
            ColumnType::from_str("decimal(precision=20, scale=6)").expect("parse decimal named");
        let spec = parsed
            .decimal_spec()
            .expect("decimal spec present after parsing");
        assert_eq!(spec.precision, 20);
        assert_eq!(spec.scale, 6);
    }

    #[test]
    fn parse_decimal_type_rejects_missing_scale() {
        let err = ColumnType::from_str("decimal(10)").expect_err("missing scale error");
        assert!(
            err.to_string()
                .contains("Decimal type requires a scale value")
        );
    }

    #[test]
    fn decimal_cli_token_formats_precision_and_scale() {
        let parsed = ColumnType::from_str("decimal(28,9)").expect("parse decimal for cli token");
        assert_eq!(parsed.cli_token(), "decimal(28,9)");
        assert_eq!(parsed.signature_token(), "decimal(28,9)");
        assert_eq!(parsed.describe(), "decimal(precision=28,scale=9)");
    }

    #[test]
    fn datatype_mappings_convert_string_to_decimal_with_rounding() {
        let spec = DecimalSpec::new(12, 2).expect("valid decimal spec");
        let mapping = DatatypeMapping {
            from: ColumnType::String,
            to: ColumnType::Decimal(spec.clone()),
            strategy: Some("round".to_string()),
            options: BTreeMap::new(),
        };
        let column = ColumnMeta {
            name: "amount".to_string(),
            datatype: ColumnType::Decimal(spec.clone()),
            rename: None,
            value_replacements: Vec::new(),
            datatype_mappings: vec![mapping],
        };
        let schema = Schema {
            columns: vec![column],
            schema_version: None,
        };
        let mut row = vec!["123.455".to_string()];
        schema
            .apply_transformations_to_row(&mut row)
            .expect("apply decimal rounding mapping");
        assert_eq!(row[0], "123.46");
    }

    #[test]
    fn datatype_mappings_convert_string_to_decimal_with_truncation() {
        let spec = DecimalSpec::new(14, 3).expect("valid decimal spec");
        let mapping = DatatypeMapping {
            from: ColumnType::String,
            to: ColumnType::Decimal(spec.clone()),
            strategy: Some("truncate".to_string()),
            options: BTreeMap::new(),
        };
        let column = ColumnMeta {
            name: "measurement".to_string(),
            datatype: ColumnType::Decimal(spec.clone()),
            rename: None,
            value_replacements: Vec::new(),
            datatype_mappings: vec![mapping],
        };
        let schema = Schema {
            columns: vec![column],
            schema_version: None,
        };
        let mut row = vec!["-87.6549".to_string()];
        schema
            .apply_transformations_to_row(&mut row)
            .expect("apply decimal truncation mapping");
        assert_eq!(row[0], "-87.654");
    }
}
