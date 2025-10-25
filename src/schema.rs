use std::{borrow::Cow, collections::BTreeMap, fmt, fs::File, io::BufReader, path::Path};

use anyhow::{Context, Result, anyhow, bail, ensure};
use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use encoding_rs::Encoding;
use serde::{Deserialize, Serialize};
use serde_yaml::Value;
use uuid::Uuid;

use crate::{
    data::{
        Value as DataValue, parse_naive_date, parse_naive_datetime, parse_naive_time,
        parse_typed_value,
    },
    io_utils,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ColumnType {
    String,
    Integer,
    Float,
    Boolean,
    Date,
    DateTime,
    Time,
    Guid,
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
        }
    }

    pub fn variants() -> &'static [&'static str] {
        &[
            "string", "integer", "float", "boolean", "date", "datetime", "time", "guid",
        ]
    }
}

impl fmt::Display for ColumnType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
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
            _ => Err(anyhow!(
                "Unknown column type '{value}'. Supported types: {}",
                ColumnType::variants().join(", ")
            )),
        }
    }
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
                        ColumnType::Float | ColumnType::Integer | ColumnType::String
                    ),
                    "Column '{}' mapping {} -> {} cannot apply 'round' strategy",
                    column_name,
                    mapping.from,
                    mapping.to
                );
            }
            if normalized == "truncate" {
                ensure!(
                    mapping.to == ColumnType::Integer,
                    "Column '{}' mapping {} -> {} cannot apply 'truncate' strategy",
                    column_name,
                    mapping.from,
                    mapping.to
                );
            }
        }
    }

    if let Some(scale) = mapping.options.get("scale") {
        if let Some(value) = scale.as_i64() {
            ensure!(
                value >= 0,
                "Column '{}' mapping {} -> {} requires a non-negative scale",
                column_name,
                mapping.from,
                mapping.to
            );
        } else if scale.as_u64().is_none() {
            bail!(
                "Column '{}' mapping {} -> {} requires 'scale' to be a number",
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

    Ok(())
}

#[derive(Debug, Clone)]
struct TypeCandidate {
    possible_integer: bool,
    possible_float: bool,
    possible_boolean: bool,
    possible_date: bool,
    possible_datetime: bool,
    possible_time: bool,
    possible_guid: bool,
}

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
            possible_integer: true,
            possible_float: true,
            possible_boolean: true,
            possible_date: true,
            possible_datetime: true,
            possible_time: true,
            possible_guid: true,
        }
    }

    fn update(&mut self, value: &str) {
        if self.possible_boolean
            && !matches!(
                value.to_ascii_lowercase().as_str(),
                "true" | "false" | "t" | "f" | "yes" | "no" | "y" | "n"
            )
        {
            self.possible_boolean = false;
        }
        if self.possible_integer && value.parse::<i64>().is_err() {
            self.possible_integer = false;
        }
        if self.possible_float && value.parse::<f64>().is_err() {
            self.possible_float = false;
        }
        if self.possible_date && parse_naive_date(value).is_err() {
            self.possible_date = false;
        }
        if self.possible_datetime && parse_naive_datetime(value).is_err() {
            self.possible_datetime = false;
        }
        if self.possible_time && parse_naive_time(value).is_err() {
            self.possible_time = false;
        }
        if self.possible_guid {
            let trimmed = value.trim().trim_matches(|c| matches!(c, '{' | '}'));
            if Uuid::parse_str(trimmed).is_err() {
                self.possible_guid = false;
            }
        }
    }

    fn decide(&self) -> ColumnType {
        if self.possible_boolean {
            ColumnType::Boolean
        } else if self.possible_integer {
            ColumnType::Integer
        } else if self.possible_float {
            ColumnType::Float
        } else if self.possible_date {
            ColumnType::Date
        } else if self.possible_datetime {
            ColumnType::DateTime
        } else if self.possible_time {
            ColumnType::Time
        } else if self.possible_guid {
            ColumnType::Guid
        } else {
            ColumnType::String
        }
    }
}

pub fn infer_schema(
    path: &Path,
    sample_rows: usize,
    delimiter: u8,
    encoding: &'static Encoding,
) -> Result<Schema> {
    let (schema, _stats) = infer_schema_with_stats(path, sample_rows, delimiter, encoding)?;
    Ok(schema)
}

pub fn infer_schema_with_stats(
    path: &Path,
    sample_rows: usize,
    delimiter: u8,
    encoding: &'static Encoding,
) -> Result<(Schema, InferenceStats)> {
    let mut reader = io_utils::open_csv_reader_from_path(path, delimiter, true)?;
    let header_record = reader.byte_headers()?.clone();
    let headers = io_utils::decode_headers(&header_record, encoding)?;
    let mut candidates = vec![TypeCandidate::new(); headers.len()];
    let mut samples = vec![None; headers.len()];
    let mut summaries = vec![SummaryAccumulator::default(); headers.len()];

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
}

#[cfg(test)]
mod tests {
    use super::*;
    use encoding_rs::UTF_8;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn infer_schema_with_stats_captures_samples() {
        let mut file = NamedTempFile::new().expect("temp file");
        writeln!(file, "id,date,value").unwrap();
        writeln!(file, "1,2024-01-01T08:30:00Z,$12.34").unwrap();
        writeln!(file, "2,2024-01-02T09:45:00Z,$56.78").unwrap();

        let (schema, stats) =
            infer_schema_with_stats(file.path(), 0, b',', UTF_8).expect("infer with stats");

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
}
