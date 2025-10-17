use std::{fs::File, io::BufReader, path::Path};

use anyhow::{Context, Result};
use csv::StringRecord;
use serde::{Deserialize, Serialize};

use crate::data::{parse_naive_date, parse_naive_datetime};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ColumnType {
    String,
    Integer,
    Float,
    Boolean,
    Date,
    DateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnMeta {
    pub name: String,
    pub data_type: ColumnType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Schema {
    pub columns: Vec<ColumnMeta>,
}

impl Schema {
    pub fn from_headers(headers: &StringRecord) -> Self {
        let columns = headers
            .iter()
            .map(|name| ColumnMeta {
                name: name.to_string(),
                data_type: ColumnType::String,
            })
            .collect();
        Schema { columns }
    }

    pub fn column_index(&self, name: &str) -> Option<usize> {
        self.columns.iter().position(|c| c.name == name)
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let file = File::create(path).with_context(|| format!("Creating meta file {path:?}"))?;
        serde_json::to_writer_pretty(file, self).context("Writing metadata JSON")
    }

    pub fn load(path: &Path) -> Result<Self> {
        let file = File::open(path).with_context(|| format!("Opening meta file {path:?}"))?;
        let reader = BufReader::new(file);
        let schema = serde_json::from_reader(reader).context("Parsing metadata JSON")?;
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
}

impl TypeCandidate {
    fn new() -> Self {
        Self {
            possible_integer: true,
            possible_float: true,
            possible_boolean: true,
            possible_date: true,
            possible_datetime: true,
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
        } else {
            ColumnType::String
        }
    }
}

pub fn infer_schema(path: &Path, sample_rows: usize, delimiter: u8) -> Result<Schema> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .delimiter(delimiter)
        .from_path(path)
        .with_context(|| format!("Opening CSV file {path:?}"))?;
    let headers = reader.headers()?.clone();
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
            let as_str = std::str::from_utf8(field)?;
            let candidate = &mut candidates[idx];
            if candidate.possible_boolean
                && !matches!(
                    as_str.to_ascii_lowercase().as_str(),
                    "true" | "false" | "t" | "f" | "yes" | "no" | "y" | "n"
                )
            {
                candidate.possible_boolean = false;
            }
            if candidate.possible_integer && as_str.parse::<i64>().is_err() {
                candidate.possible_integer = false;
            }
            if candidate.possible_float && as_str.parse::<f64>().is_err() {
                candidate.possible_float = false;
            }
            if candidate.possible_date && parse_naive_date(as_str).is_err() {
                candidate.possible_date = false;
            }
            if candidate.possible_datetime && parse_naive_datetime(as_str).is_err() {
                candidate.possible_datetime = false;
            }
        }
        processed += 1;
    }

    let columns = headers
        .iter()
        .enumerate()
        .map(|(idx, header)| ColumnMeta {
            name: header.to_string(),
            data_type: candidates[idx].decide(),
        })
        .collect();

    Ok(Schema { columns })
}
