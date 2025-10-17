use std::{
    collections::BTreeMap,
    fs::File,
    io::{BufReader, BufWriter},
    path::Path,
};

use anyhow::{Context, Result, anyhow};
use csv::StringRecord;
use serde::{Deserialize, Serialize};

use crate::{
    data::{ComparableValue, parse_typed_value},
    metadata::{ColumnType, Schema},
};

const INDEX_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CsvIndex {
    version: u32,
    pub columns: Vec<String>,
    pub column_types: Vec<ColumnType>,
    pub headers: Vec<String>,
    map: BTreeMap<Vec<ComparableValue>, Vec<u64>>,
}

impl CsvIndex {
    pub fn build(
        csv_path: &Path,
        columns: &[String],
        schema: Option<&Schema>,
        limit: Option<usize>,
        delimiter: u8,
    ) -> Result<Self> {
        if columns.is_empty() {
            return Err(anyhow!("At least one column is required to build an index"));
        }
        let mut reader = csv::ReaderBuilder::new()
            .has_headers(true)
            .delimiter(delimiter)
            .from_path(csv_path)
            .with_context(|| format!("Opening CSV file {csv_path:?}"))?;
        let headers = reader.headers()?.clone();
        let column_indices = lookup_indices(&headers, columns)?;
        let column_types = columns
            .iter()
            .map(|name| {
                schema
                    .and_then(|s| s.columns.iter().find(|c| c.name == *name))
                    .map(|c| c.data_type.clone())
                    .unwrap_or(ColumnType::String)
            })
            .collect::<Vec<_>>();

        let mut record = csv::ByteRecord::new();
        let mut map: BTreeMap<Vec<ComparableValue>, Vec<u64>> = BTreeMap::new();
        let mut processed = 0usize;

        while {
            if let Some(limit) = limit {
                if processed >= limit { false } else { true }
            } else {
                true
            }
        } {
            let start_offset = reader.position().byte();
            if !reader.read_byte_record(&mut record)? {
                break;
            }
            let mut key_components = Vec::with_capacity(columns.len());
            for (pos, column_index) in column_indices.iter().enumerate() {
                let raw = record
                    .get(*column_index)
                    .map(|slice| std::str::from_utf8(slice))
                    .transpose()
                    .context("Decoding UTF-8 while building index")?;
                let component = match raw {
                    Some(raw_value) => {
                        let ty = &column_types[pos];
                        let parsed = parse_typed_value(raw_value, ty)?;
                        ComparableValue(parsed)
                    }
                    None => ComparableValue(None),
                };
                key_components.push(component);
            }
            map.entry(key_components)
                .or_insert_with(Vec::new)
                .push(start_offset);
            processed += 1;
        }

        Ok(CsvIndex {
            version: INDEX_VERSION,
            columns: columns.to_vec(),
            column_types,
            headers: headers.iter().map(|s| s.to_string()).collect(),
            map,
        })
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let file = File::create(path).with_context(|| format!("Creating index file {path:?}"))?;
        let writer = BufWriter::new(file);
        bincode::serialize_into(writer, self).context("Writing index file")
    }

    pub fn load(path: &Path) -> Result<Self> {
        let file = File::open(path).with_context(|| format!("Opening index file {path:?}"))?;
        let reader = BufReader::new(file);
        let index: CsvIndex = bincode::deserialize_from(reader).context("Reading index file")?;
        if index.version != INDEX_VERSION {
            return Err(anyhow!(
                "Unsupported index version {} (expected {INDEX_VERSION})",
                index.version
            ));
        }
        Ok(index)
    }

    pub fn ordered_offsets(&self) -> impl Iterator<Item = u64> + '_ {
        self.map
            .values()
            .flat_map(|offsets| offsets.iter().copied())
    }

    pub fn supports_sort(&self, directives: &[(String, bool)]) -> bool {
        if directives.len() < self.columns.len() {
            return false;
        }
        self.columns
            .iter()
            .zip(directives.iter())
            .all(|(index_col, (sort_col, ascending))| index_col == sort_col && *ascending)
    }

    pub fn row_count(&self) -> usize {
        self.map.values().map(|offsets| offsets.len()).sum()
    }
}

fn lookup_indices(headers: &StringRecord, columns: &[String]) -> Result<Vec<usize>> {
    columns
        .iter()
        .map(|column| {
            headers
                .iter()
                .position(|header| header == column)
                .ok_or_else(|| anyhow!("Column '{column}' not found in CSV headers"))
        })
        .collect()
}
