use std::{collections::BTreeMap, fs::File, io::BufWriter, path::Path};

use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};

use crate::{
    data::{ComparableValue, parse_typed_value},
    io_utils,
    metadata::{ColumnType, Schema},
};

use encoding_rs::Encoding;

const INDEX_VERSION: u32 = 2;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum SortDirection {
    Asc,
    Desc,
}

impl SortDirection {
    pub fn is_ascending(self) -> bool {
        matches!(self, SortDirection::Asc)
    }

    fn as_str(self) -> &'static str {
        match self {
            SortDirection::Asc => "asc",
            SortDirection::Desc => "desc",
        }
    }
}

impl std::fmt::Display for SortDirection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Debug, Clone)]
pub struct IndexDefinition {
    pub columns: Vec<String>,
    pub directions: Vec<SortDirection>,
    pub name: Option<String>,
}

impl IndexDefinition {
    pub fn from_columns(columns: Vec<String>) -> Result<Self> {
        let cleaned: Vec<String> = columns
            .into_iter()
            .map(|c| c.trim().to_string())
            .filter(|c| !c.is_empty())
            .collect();
        if cleaned.is_empty() {
            return Err(anyhow!("At least one column is required to build an index"));
        }
        Ok(IndexDefinition {
            directions: vec![SortDirection::Asc; cleaned.len()],
            columns: cleaned,
            name: None,
        })
    }

    pub fn parse(spec: &str) -> Result<Self> {
        let (name, remainder) = if let Some((raw_name, rest)) = spec.split_once('=') {
            let trimmed_name = raw_name.trim();
            if trimmed_name.is_empty() {
                return Err(anyhow!(
                    "Index specification is missing a variant name before '=': '{spec}'"
                ));
            }
            let trimmed_rest = rest.trim();
            if trimmed_rest.is_empty() {
                return Err(anyhow!(
                    "Index specification '{spec}' is missing column definitions after '='"
                ));
            }
            (Some(trimmed_name.to_string()), trimmed_rest)
        } else {
            (None, spec)
        };

        let mut columns = Vec::new();
        let mut directions = Vec::new();
        for token in remainder.split(',') {
            let mut parts = token.split(':');
            let column = parts
                .next()
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .ok_or_else(|| anyhow!("Index specification is missing a column name"))?;
            let direction = parts
                .next()
                .map(|raw| raw.trim().to_ascii_lowercase())
                .filter(|s| !s.is_empty())
                .map(|value| match value.as_str() {
                    "asc" => Ok(SortDirection::Asc),
                    "desc" => Ok(SortDirection::Desc),
                    other => Err(anyhow!("Unknown sort direction '{other}'")),
                })
                .transpose()?;
            columns.push(column.to_string());
            directions.push(direction.unwrap_or(SortDirection::Asc));
        }
        if columns.is_empty() {
            return Err(anyhow!(
                "Index specification did not contain any columns: '{spec}'"
            ));
        }
        Ok(IndexDefinition {
            columns,
            directions,
            name,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CsvIndex {
    version: u32,
    headers: Vec<String>,
    variants: Vec<IndexVariant>,
    row_count: usize,
}

impl CsvIndex {
    pub fn build(
        csv_path: &Path,
        definitions: &[IndexDefinition],
        schema: Option<&Schema>,
        limit: Option<usize>,
        delimiter: u8,
        encoding: &'static Encoding,
    ) -> Result<Self> {
        if definitions.is_empty() {
            return Err(anyhow!(
                "Specify at least one column set via --columns or --spec"
            ));
        }

        let mut reader = io_utils::open_seekable_csv_reader(csv_path, delimiter, true)?;
        let headers = io_utils::reader_headers(&mut reader, encoding)?;

        let mut builders = definitions
            .iter()
            .map(|definition| IndexVariantBuilder::new(definition, &headers, schema, encoding))
            .collect::<Result<Vec<_>>>()?;

        let mut record = csv::ByteRecord::new();
        let mut processed = 0usize;

        loop {
            if let Some(limit) = limit {
                if processed >= limit {
                    break;
                }
            }
            let start_offset = reader.position().byte();
            if !reader.read_byte_record(&mut record)? {
                break;
            }
            for builder in builders.iter_mut() {
                builder.add_record(&record, start_offset)?;
            }
            processed += 1;
        }

        let variants = builders
            .into_iter()
            .map(IndexVariantBuilder::finish)
            .collect::<Vec<_>>();

        Ok(CsvIndex {
            version: INDEX_VERSION,
            headers,
            row_count: processed,
            variants,
        })
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let file = File::create(path).with_context(|| format!("Creating index file {path:?}"))?;
        let writer = BufWriter::new(file);
        bincode::serialize_into(writer, self).context("Writing index file")
    }

    pub fn load(path: &Path) -> Result<Self> {
        let bytes = std::fs::read(path).with_context(|| format!("Opening index file {path:?}"))?;
        match bincode::deserialize::<CsvIndex>(&bytes) {
            Ok(index) => {
                if index.version != INDEX_VERSION {
                    return Err(anyhow!(
                        "Unsupported index version {} (expected {INDEX_VERSION})",
                        index.version
                    ));
                }
                Ok(index)
            }
            Err(_) => {
                let legacy: LegacyCsvIndex =
                    bincode::deserialize(&bytes).context("Reading legacy index file format")?;
                Ok(legacy.into())
            }
        }
    }

    pub fn variants(&self) -> &[IndexVariant] {
        &self.variants
    }

    pub fn row_count(&self) -> usize {
        self.row_count
    }

    pub fn variant_by_name(&self, name: &str) -> Option<&IndexVariant> {
        self.variants
            .iter()
            .find(|variant| variant.name.as_deref() == Some(name))
    }

    pub fn best_match(&self, directives: &[(String, SortDirection)]) -> Option<&IndexVariant> {
        let mut best: Option<&IndexVariant> = None;
        for variant in &self.variants {
            if variant.matches(directives) {
                let replace = match best {
                    None => true,
                    Some(current) => variant.columns.len() > current.columns.len(),
                };
                if replace {
                    best = Some(variant);
                }
            }
        }
        best
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexVariant {
    columns: Vec<String>,
    directions: Vec<SortDirection>,
    column_types: Vec<ColumnType>,
    map: BTreeMap<Vec<DirectionalComparableValue>, Vec<u64>>,
    #[serde(default)]
    name: Option<String>,
}

impl IndexVariant {
    pub fn columns(&self) -> &[String] {
        &self.columns
    }

    pub fn directions(&self) -> &[SortDirection] {
        &self.directions
    }

    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    pub fn column_types(&self) -> &[ColumnType] {
        &self.column_types
    }

    pub fn ordered_offsets(&self) -> impl Iterator<Item = u64> + '_ {
        self.map
            .values()
            .flat_map(|offsets| offsets.iter().copied())
    }

    pub fn matches(&self, directives: &[(String, SortDirection)]) -> bool {
        if directives.len() < self.columns.len() {
            return false;
        }
        self.columns
            .iter()
            .zip(self.directions.iter())
            .zip(directives.iter())
            .all(
                |((column, direction), (requested_column, requested_direction))| {
                    column == requested_column && direction == requested_direction
                },
            )
    }

    pub fn describe(&self) -> String {
        let body = self
            .columns
            .iter()
            .zip(self.directions.iter())
            .map(|(column, direction)| format!("{column}:{}", direction))
            .collect::<Vec<_>>()
            .join(", ");
        match &self.name {
            Some(name) => format!("{name} -> {body}"),
            None => body,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct DirectionalComparableValue {
    value: ComparableValue,
    direction: SortDirection,
}

impl DirectionalComparableValue {
    fn new(value: ComparableValue, direction: SortDirection) -> Self {
        Self { value, direction }
    }
}

impl std::cmp::Ord for DirectionalComparableValue {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        debug_assert_eq!(self.direction, other.direction);
        match self.direction {
            SortDirection::Asc => self.value.cmp(&other.value),
            SortDirection::Desc => other.value.cmp(&self.value),
        }
    }
}

impl PartialOrd for DirectionalComparableValue {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

struct IndexVariantBuilder {
    columns: Vec<String>,
    directions: Vec<SortDirection>,
    column_indices: Vec<usize>,
    column_types: Vec<ColumnType>,
    map: BTreeMap<Vec<DirectionalComparableValue>, Vec<u64>>,
    encoding: &'static Encoding,
    name: Option<String>,
}

impl IndexVariantBuilder {
    fn new(
        definition: &IndexDefinition,
        headers: &[String],
        schema: Option<&Schema>,
        encoding: &'static Encoding,
    ) -> Result<Self> {
        if definition.columns.len() != definition.directions.len() {
            return Err(anyhow!(
                "Column count and direction count mismatch for index specification"
            ));
        }
        let column_indices = lookup_indices(headers, &definition.columns)?;
        let column_types = definition
            .columns
            .iter()
            .map(|name| {
                schema
                    .and_then(|s| s.columns.iter().find(|c| c.name == *name))
                    .map(|c| c.data_type.clone())
                    .unwrap_or(ColumnType::String)
            })
            .collect();
        Ok(IndexVariantBuilder {
            columns: definition.columns.clone(),
            directions: definition.directions.clone(),
            column_indices,
            column_types,
            map: BTreeMap::new(),
            encoding,
            name: definition.name.clone(),
        })
    }

    fn add_record(&mut self, record: &csv::ByteRecord, offset: u64) -> Result<()> {
        let mut key_components = Vec::with_capacity(self.column_indices.len());
        for (idx, column_index) in self.column_indices.iter().enumerate() {
            let raw = record
                .get(*column_index)
                .map(|slice| io_utils::decode_bytes(slice, self.encoding))
                .transpose()?;
            let comparable = match raw {
                Some(value) => {
                    let ty = &self.column_types[idx];
                    let parsed = parse_typed_value(&value, ty)?;
                    ComparableValue(parsed)
                }
                None => ComparableValue(None),
            };
            key_components.push(DirectionalComparableValue::new(
                comparable,
                self.directions[idx],
            ));
        }
        self.map
            .entry(key_components)
            .or_insert_with(Vec::new)
            .push(offset);
        Ok(())
    }

    fn finish(self) -> IndexVariant {
        IndexVariant {
            columns: self.columns,
            directions: self.directions,
            column_types: self.column_types,
            map: self.map,
            name: self.name,
        }
    }
}

fn lookup_indices(headers: &[String], columns: &[String]) -> Result<Vec<usize>> {
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

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LegacyCsvIndex {
    version: u32,
    columns: Vec<String>,
    column_types: Vec<ColumnType>,
    headers: Vec<String>,
    map: BTreeMap<Vec<ComparableValue>, Vec<u64>>,
}

impl From<LegacyCsvIndex> for CsvIndex {
    fn from(legacy: LegacyCsvIndex) -> Self {
        let directions = vec![SortDirection::Asc; legacy.columns.len()];
        let map = legacy
            .map
            .into_iter()
            .map(|(key, offsets)| {
                let directional_key = key
                    .into_iter()
                    .map(|value| DirectionalComparableValue::new(value, SortDirection::Asc))
                    .collect::<Vec<_>>();
                (directional_key, offsets)
            })
            .collect::<BTreeMap<_, _>>();
        let row_count = map.values().map(|offsets| offsets.len()).sum();
        CsvIndex {
            version: INDEX_VERSION,
            headers: legacy.headers,
            variants: vec![IndexVariant {
                columns: legacy.columns,
                directions,
                column_types: legacy.column_types,
                map,
                name: None,
            }],
            row_count,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use encoding_rs::UTF_8;
    use tempfile::tempdir;

    #[test]
    fn parse_index_spec_supports_mixed_directions() {
        let spec = IndexDefinition::parse("col1:desc,col2:asc,col3").unwrap();
        assert_eq!(spec.columns, vec!["col1", "col2", "col3"]);
        assert_eq!(
            spec.directions,
            vec![SortDirection::Desc, SortDirection::Asc, SortDirection::Asc]
        );
        assert!(spec.name.is_none());
    }

    #[test]
    fn parse_index_spec_supports_named_variants() {
        let spec = IndexDefinition::parse("top=col1:desc,col2").unwrap();
        assert_eq!(spec.name.as_deref(), Some("top"));
        assert_eq!(spec.columns, vec!["col1", "col2"]);
        assert_eq!(
            spec.directions,
            vec![SortDirection::Desc, SortDirection::Asc]
        );
    }

    #[test]
    fn build_multiple_variants_and_match() {
        let dir = tempdir().unwrap();
        let csv_path = dir.path().join("data.csv");
        std::fs::write(&csv_path, "a,b,c\n1,x,alpha\n2,y,beta\n3,z,gamma\n").unwrap();

        let definitions = vec![
            IndexDefinition::from_columns(vec!["a".to_string()]).unwrap(),
            IndexDefinition::parse("descending=a:desc,b:asc").unwrap(),
        ];

        let index = CsvIndex::build(&csv_path, &definitions, None, None, b',', UTF_8).unwrap();

        assert_eq!(index.variants().len(), 2);

        let asc_match = index
            .best_match(&vec![("a".to_string(), SortDirection::Asc)])
            .unwrap();
        assert_eq!(
            asc_match
                .columns()
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>(),
            vec!["a"]
        );

        let desc_match = index
            .best_match(&vec![
                ("a".to_string(), SortDirection::Desc),
                ("b".to_string(), SortDirection::Asc),
            ])
            .unwrap();
        assert_eq!(desc_match.name(), Some("descending"));
        assert_eq!(
            desc_match
                .columns()
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>(),
            vec!["a", "b"]
        );

        let offsets: Vec<u64> = desc_match.ordered_offsets().collect();
        assert_eq!(offsets.len(), 3);
        // Ensure first offset corresponds to highest "a" value (3)
        assert!(offsets[0] > offsets[2]);
    }
}
