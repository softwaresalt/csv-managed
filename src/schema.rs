use std::{fmt, fs::File, io::BufReader, path::Path};

use anyhow::{Context, Result, anyhow};
use encoding_rs::Encoding;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    data::{parse_naive_date, parse_naive_datetime},
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
            ColumnType::Guid => "guid",
        }
    }

    pub fn variants() -> &'static [&'static str] {
        &[
            "string", "integer", "float", "boolean", "date", "datetime", "guid",
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
            "guid" | "uuid" => Ok(ColumnType::Guid),
            _ => Err(anyhow!(
                "Unknown column type '{value}'. Supported types: {}",
                ColumnType::variants().join(", ")
            )),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnMeta {
    pub name: String,
    pub data_type: ColumnType,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rename: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Schema {
    pub columns: Vec<ColumnMeta>,
}

impl Schema {
    pub fn from_headers(headers: &[String]) -> Self {
        let columns = headers
            .iter()
            .map(|name| ColumnMeta {
                name: name.clone(),
                data_type: ColumnType::String,
                rename: None,
            })
            .collect();
        Schema { columns }
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
        let file = File::create(path).with_context(|| format!("Creating schema file {path:?}"))?;
        serde_json::to_writer_pretty(file, self).context("Writing schema JSON")
    }

    pub fn load(path: &Path) -> Result<Self> {
        let file = File::open(path).with_context(|| format!("Opening schema file {path:?}"))?;
        let reader = BufReader::new(file);
        let schema = serde_json::from_reader(reader).context("Parsing schema JSON")?;
        Ok(schema)
    }
}

#[derive(Debug, Clone)]
struct TypeCandidate {
    possible_integer: bool,
    possible_float: bool,
    possible_boolean: bool,
    possible_date: bool,
    possible_datetime: bool,
    possible_guid: bool,
}

impl TypeCandidate {
    fn new() -> Self {
        Self {
            possible_integer: true,
            possible_float: true,
            possible_boolean: true,
            possible_date: true,
            possible_datetime: true,
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
    let mut reader = io_utils::open_csv_reader_from_path(path, delimiter, true)?;
    let header_record = reader.byte_headers()?.clone();
    let headers = io_utils::decode_headers(&header_record, encoding)?;
    let mut candidates = vec![TypeCandidate::new(); headers.len()];

    let mut record = csv::ByteRecord::new();
    let mut processed = 0usize;
    while reader.read_byte_record(&mut record)? {
        if sample_rows > 0 && processed >= sample_rows {
            break;
        }
        for (idx, field) in record.iter().enumerate() {
            if field.is_empty() {
                continue;
            }
            let decoded = io_utils::decode_bytes(field, encoding)?;
            candidates[idx].update(&decoded);
        }
        processed += 1;
    }

    let columns = headers
        .iter()
        .enumerate()
        .map(|(idx, header)| ColumnMeta {
            name: header.clone(),
            data_type: candidates[idx].decide(),
            rename: None,
        })
        .collect();

    Ok(Schema { columns })
}

impl ColumnMeta {
    pub fn output_name(&self) -> &str {
        self.rename
            .as_deref()
            .filter(|value| !value.is_empty())
            .unwrap_or(&self.name)
    }
}
