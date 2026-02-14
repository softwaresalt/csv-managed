//! B-tree index construction, serialization, and variant selection.
//!
//! Builds one or more sorted index variants over a CSV file, enabling seek-based
//! row retrieval in sorted order without buffering the entire dataset. Supports
//! named variants, covering-index expansion, per-column sort direction, versioned
//! binary serialization via `bincode`, and longest-prefix best-match selection.
//!
//! # Complexity
//!
//! Index build is O(n log n) per variant where n is the row count. Variant
//! selection and ordered-offset iteration are O(v) and O(n) respectively.

use std::{borrow::Cow, collections::BTreeMap, fs::File, io::BufWriter, path::Path};

use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};

use crate::{
    data::{ComparableValue, parse_typed_value},
    io_utils,
    schema::{ColumnMeta, ColumnType, Schema},
};

use encoding_rs::Encoding;

const INDEX_VERSION: u32 = 2;

/// Sort order for an indexed column — ascending or descending.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum SortDirection {
    /// Ascending (smallest first).
    Asc,
    /// Descending (largest first).
    Desc,
}

impl SortDirection {
    /// Returns `true` when the direction is [`Asc`](SortDirection::Asc).
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

/// Describes which columns and sort directions to include in a single index variant.
#[derive(Debug, Clone)]
pub struct IndexDefinition {
    pub columns: Vec<String>,
    pub directions: Vec<SortDirection>,
    pub name: Option<String>,
}

impl IndexDefinition {
    /// Creates an index definition from column names, defaulting every direction to ascending.
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

    /// Parses a `name=col1:dir,col2:dir` specification string into an [`IndexDefinition`].
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

    /// Expands a covering specification into all prefix-length and direction-product index variants.
    pub fn expand_covering_spec(spec: &str) -> Result<Vec<Self>> {
        let (name_prefix, remainder) = if let Some((raw_name, rest)) = spec.split_once('=') {
            let trimmed_name = raw_name.trim();
            if trimmed_name.is_empty() {
                return Err(anyhow!(
                    "Covering specification is missing a name before '=': '{spec}'"
                ));
            }
            let trimmed_rest = rest.trim();
            if trimmed_rest.is_empty() {
                return Err(anyhow!(
                    "Covering specification '{spec}' is missing column definitions after '='"
                ));
            }
            (Some(trimmed_name.to_string()), trimmed_rest)
        } else {
            (None, spec.trim())
        };

        let columns = remainder
            .split(',')
            .map(|token| token.trim())
            .filter(|token| !token.is_empty())
            .map(parse_covering_column)
            .collect::<Result<Vec<_>>>()?;

        if columns.is_empty() {
            return Err(anyhow!(
                "Covering specification did not contain any columns: '{spec}'"
            ));
        }

        let mut definitions = Vec::new();
        for prefix_len in 1..=columns.len() {
            let prefix = &columns[..prefix_len];
            let direction_sets = prefix
                .iter()
                .map(|column| column.directions.as_slice())
                .collect::<Vec<_>>();
            for directions in cartesian_product(&direction_sets) {
                let column_names = prefix
                    .iter()
                    .map(|column| column.name.clone())
                    .collect::<Vec<_>>();
                let variant_name =
                    build_covering_name(name_prefix.as_deref(), &column_names, &directions);
                definitions.push(IndexDefinition {
                    columns: column_names,
                    directions,
                    name: Some(variant_name),
                });
            }
        }

        Ok(definitions)
    }
}

#[derive(Debug, Clone)]
struct CoveringColumn {
    name: String,
    directions: Vec<SortDirection>,
}

fn parse_covering_column(token: &str) -> Result<CoveringColumn> {
    let mut parts = token.split(':');
    let name = parts
        .next()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| anyhow!("Covering specification is missing a column name"))?;
    let directions = if let Some(dir_part) = parts.next() {
        let options = dir_part
            .split('|')
            .map(|raw| raw.trim().to_ascii_lowercase())
            .filter(|s| !s.is_empty())
            .map(|value| match value.as_str() {
                "asc" => Ok(SortDirection::Asc),
                "desc" => Ok(SortDirection::Desc),
                other => Err(anyhow!("Unknown sort direction '{other}'")),
            })
            .collect::<Result<Vec<_>>>()?;
        if options.is_empty() {
            vec![SortDirection::Asc]
        } else {
            options
        }
    } else {
        vec![SortDirection::Asc]
    };

    Ok(CoveringColumn {
        name: name.to_string(),
        directions,
    })
}

fn cartesian_product(options: &[&[SortDirection]]) -> Vec<Vec<SortDirection>> {
    let mut acc = vec![Vec::new()];
    for set in options {
        let mut next = Vec::new();
        for combination in &acc {
            for direction in *set {
                let mut updated = combination.clone();
                updated.push(*direction);
                next.push(updated);
            }
        }
        acc = next;
    }
    acc
}

fn build_covering_name(
    prefix: Option<&str>,
    columns: &[String],
    directions: &[SortDirection],
) -> String {
    let suffix = columns
        .iter()
        .zip(directions.iter())
        .map(|(column, direction)| {
            format!("{}-{}", sanitize_identifier(column), direction.as_str())
        })
        .collect::<Vec<_>>()
        .join("_");
    match prefix {
        Some(p) => {
            if suffix.is_empty() {
                sanitize_identifier(p)
            } else {
                format!("{}_{}", sanitize_identifier(p), suffix)
            }
        }
        None => suffix,
    }
}

fn sanitize_identifier(value: &str) -> String {
    value
        .chars()
        .map(|ch| match ch {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '_' => ch,
            _ => '_',
        })
        .collect()
}

/// Serializable B-tree index over a CSV file, containing one or more sorted variants.
///
/// Each variant maps composite typed keys to byte offsets within the source CSV,
/// enabling seek-based sorted reads without loading the full dataset.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CsvIndex {
    version: u32,
    headers: Vec<String>,
    variants: Vec<IndexVariant>,
    row_count: usize,
}

impl CsvIndex {
    /// Builds an in-memory index by streaming every row and inserting typed keys into B-tree maps.
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
            if limit.is_some_and(|limit| processed >= limit) {
                break;
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

    /// Serializes the index to a binary file using `bincode`.
    pub fn save(&self, path: &Path) -> Result<()> {
        let file = File::create(path).with_context(|| format!("Creating index file {path:?}"))?;
        let mut writer = BufWriter::new(file);
        bincode::serde::encode_into_std_write(self, &mut writer, bincode::config::legacy())
            .context("Writing index file")?;
        Ok(())
    }

    /// Deserializes an index from a binary file, with fallback to legacy format migration.
    pub fn load(path: &Path) -> Result<Self> {
        let bytes = std::fs::read(path).with_context(|| format!("Opening index file {path:?}"))?;
        let config = bincode::config::legacy();
        match bincode::serde::decode_from_slice::<CsvIndex, _>(&bytes, config) {
            Ok((index, _)) => {
                if index.version != INDEX_VERSION {
                    return Err(anyhow!(
                        "Unsupported index version {} (expected {INDEX_VERSION})",
                        index.version
                    ));
                }
                Ok(index)
            }
            Err(err) => {
                let (legacy, _) =
                    bincode::serde::decode_from_slice::<LegacyCsvIndex, _>(&bytes, config)
                        .with_context(|| {
                            format!("Reading legacy index file format after decode error: {err}")
                        })?;
                Ok(legacy.into())
            }
        }
    }

    /// Returns a slice of all index variants stored in this index.
    pub fn variants(&self) -> &[IndexVariant] {
        &self.variants
    }

    /// Returns the total number of data rows indexed.
    pub fn row_count(&self) -> usize {
        self.row_count
    }

    /// Looks up a variant by its assigned name, returning `None` if no match exists.
    pub fn variant_by_name(&self, name: &str) -> Option<&IndexVariant> {
        self.variants
            .iter()
            .find(|variant| variant.name.as_deref() == Some(name))
    }

    /// Selects the variant whose columns and directions form the longest matching prefix
    /// of the requested sort directives.
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

/// A single sorted view within a [`CsvIndex`], mapping composite typed keys to byte offsets.
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
    /// Returns the column names that form this variant's composite key.
    pub fn columns(&self) -> &[String] {
        &self.columns
    }

    /// Returns the sort direction for each column in the composite key.
    pub fn directions(&self) -> &[SortDirection] {
        &self.directions
    }

    /// Returns the optional human-readable name assigned to this variant.
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// Returns the inferred or schema-provided data types for each key column.
    pub fn column_types(&self) -> &[ColumnType] {
        &self.column_types
    }

    /// Returns an iterator of byte offsets in sorted key order for seek-based CSV reading.
    pub fn ordered_offsets(&self) -> impl Iterator<Item = u64> + '_ {
        self.map
            .values()
            .flat_map(|offsets| offsets.iter().copied())
    }

    /// Returns `true` when this variant's columns and directions are a prefix match
    /// for the given sort directives.
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

    /// Formats a human-readable summary of the variant's columns, directions, and optional name.
    pub fn describe(&self) -> String {
        let body = self
            .columns
            .iter()
            .zip(self.directions.iter())
            .map(|(column, direction)| format!("{column}:{direction}"))
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
    column_meta: Vec<Option<ColumnMeta>>,
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
        let column_meta = definition
            .columns
            .iter()
            .map(|name| {
                schema
                    .and_then(|s| s.columns.iter().find(|c| c.name == *name))
                    .cloned()
            })
            .collect::<Vec<_>>();
        let column_types = column_meta
            .iter()
            .map(|meta| {
                meta.as_ref()
                    .map(|c| c.datatype.clone())
                    .unwrap_or(ColumnType::String)
            })
            .collect();
        Ok(IndexVariantBuilder {
            columns: definition.columns.clone(),
            directions: definition.directions.clone(),
            column_indices,
            column_types,
            column_meta,
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
                    let final_value = if let Some(meta) =
                        self.column_meta.get(idx).and_then(|meta| meta.as_ref())
                    {
                        let mut current: Cow<'_, str> = Cow::Borrowed(value.as_str());
                        if meta.has_mappings() {
                            current = match meta.apply_mappings_to_value(current.as_ref())? {
                                Some(mapped) => Cow::Owned(mapped),
                                None => Cow::Owned(String::new()),
                            };
                        }
                        current = match meta.normalize_value(current.as_ref()) {
                            Cow::Borrowed(_) => current,
                            Cow::Owned(replaced) => Cow::Owned(replaced),
                        };
                        current
                    } else {
                        Cow::Borrowed(value.as_str())
                    };
                    let parsed = parse_typed_value(final_value.as_ref(), ty)?;
                    ComparableValue(parsed)
                }
                None => ComparableValue(None),
            };
            key_components.push(DirectionalComparableValue::new(
                comparable,
                self.directions[idx],
            ));
        }
        self.map.entry(key_components).or_default().push(offset);
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
    use crate::schema::{ColumnMeta, ColumnType, DecimalSpec, Schema};
    use encoding_rs::UTF_8;
    use std::fs;
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
    fn parse_index_spec_requires_column_name() {
        let err =
            IndexDefinition::parse("col1,,col2").expect_err("spec with missing column should fail");
        assert!(err.to_string().contains("missing a column name"));
    }

    #[test]
    fn parse_index_spec_rejects_unknown_direction() {
        let err =
            IndexDefinition::parse("col1:sideways").expect_err("unknown direction should fail");
        assert!(err.to_string().contains("Unknown sort direction"));
    }

    #[test]
    fn index_definition_from_columns_rejects_empty() {
        let err = IndexDefinition::from_columns(vec![" ".to_string()])
            .expect_err("empty column list should fail");
        assert!(err.to_string().contains("At least one column"));
    }

    #[test]
    fn expand_covering_spec_generates_prefix_variants() {
        let variants = IndexDefinition::expand_covering_spec("col1:asc|desc,col2:asc").unwrap();
        assert_eq!(variants.len(), 4);
        let coverings: Vec<(Vec<String>, Vec<SortDirection>, String)> = variants
            .into_iter()
            .map(|definition| {
                (
                    definition.columns,
                    definition.directions,
                    definition.name.unwrap(),
                )
            })
            .collect();
        assert!(coverings.iter().any(|(cols, dirs, _)| {
            cols == &vec!["col1".to_string()] && dirs == &vec![SortDirection::Asc]
        }));
        assert!(coverings.iter().any(|(cols, dirs, _)| {
            cols == &vec!["col1".to_string()] && dirs == &vec![SortDirection::Desc]
        }));
        assert!(coverings.iter().any(|(cols, dirs, name)| {
            cols == &vec!["col1".to_string(), "col2".to_string()]
                && dirs == &vec![SortDirection::Asc, SortDirection::Asc]
                && name.contains("col1-asc")
        }));
    }

    #[test]
    fn save_and_load_index_with_decimal_column() {
        let temp = tempdir().expect("temp dir");
        let csv_path = temp.path().join("decimal.csv");
        fs::write(&csv_path, "id,amount\n1,42.50\n2,13.37\n").expect("write csv");

        let schema = Schema {
            columns: vec![
                ColumnMeta {
                    name: "id".to_string(),
                    datatype: ColumnType::Integer,
                    rename: None,
                    value_replacements: Vec::new(),
                    datatype_mappings: Vec::new(),
                },
                ColumnMeta {
                    name: "amount".to_string(),
                    datatype: ColumnType::Decimal(
                        DecimalSpec::new(4, 2).expect("valid decimal spec"),
                    ),
                    rename: None,
                    value_replacements: Vec::new(),
                    datatype_mappings: Vec::new(),
                },
            ],
            schema_version: None,
            has_headers: true,
        };

        let definition = IndexDefinition::from_columns(vec!["amount".to_string()]).unwrap();
        let index = CsvIndex::build(&csv_path, &[definition], Some(&schema), None, b',', UTF_8)
            .expect("build index");

        let index_path = temp.path().join("decimal.idx");
        index.save(&index_path).expect("save index");

        let loaded = CsvIndex::load(&index_path).expect("load index");
        assert_eq!(loaded.variants().len(), index.variants().len());
        assert_eq!(loaded.row_count(), index.row_count());
    }

    #[test]
    fn expand_covering_spec_honors_name_prefix() {
        let variants =
            IndexDefinition::expand_covering_spec("geo=country:asc|desc,region:asc|desc").unwrap();
        assert!(variants.len() >= 4);
        for definition in variants {
            let name = definition.name.unwrap();
            assert!(name.starts_with("geo_"));
            assert_eq!(definition.columns[0], "country");
        }
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
            .best_match(&[("a".to_string(), SortDirection::Asc)])
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
            .best_match(&[
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

    /// FR-037: When sort has more columns than any single variant, the longest
    /// matching prefix is selected (true partial match scenario).
    #[test]
    fn best_match_selects_longest_prefix_variant() {
        let dir = tempdir().unwrap();
        let csv_path = dir.path().join("data.csv");
        std::fs::write(&csv_path, "a,b,c\n1,x,alpha\n2,y,beta\n3,z,gamma\n").unwrap();

        let definitions = vec![
            IndexDefinition::parse("short=a:asc").unwrap(),
            IndexDefinition::parse("long=a:asc,b:asc").unwrap(),
        ];

        let index = CsvIndex::build(&csv_path, &definitions, None, None, b',', UTF_8).unwrap();
        assert_eq!(index.variants().len(), 2);

        // Sort by (a:asc, b:asc, c:asc) — both variants match as prefix, but
        // "long" covers 2 columns vs "short" covering 1, so "long" wins.
        let matched = index
            .best_match(&[
                ("a".to_string(), SortDirection::Asc),
                ("b".to_string(), SortDirection::Asc),
                ("c".to_string(), SortDirection::Asc),
            ])
            .expect("should find a matching variant");
        assert_eq!(matched.name(), Some("long"));
        assert_eq!(matched.columns().len(), 2);
    }

    /// FR-039: Loading an index with a mismatched version returns a clear error.
    #[test]
    fn load_rejects_incompatible_index_version() {
        let dir = tempdir().unwrap();
        let csv_path = dir.path().join("data.csv");
        std::fs::write(&csv_path, "a\n1\n2\n").unwrap();

        let definition = IndexDefinition::from_columns(vec!["a".to_string()]).unwrap();
        let mut index = CsvIndex::build(&csv_path, &[definition], None, None, b',', UTF_8).unwrap();

        // Tamper with the version to simulate a future incompatible format.
        index.version = INDEX_VERSION + 99;
        let index_path = dir.path().join("bad_version.idx");
        index.save(&index_path).expect("save tampered index");

        let err = CsvIndex::load(&index_path).expect_err("should reject incompatible version");
        let msg = err.to_string();
        assert!(
            msg.contains("Unsupported index version") || msg.contains("index"),
            "Error should mention version incompatibility, got: {msg}"
        );
    }

    #[test]
    fn expand_covering_spec_rejects_empty_spec() {
        let err =
            IndexDefinition::expand_covering_spec("").expect_err("empty covering spec should fail");
        assert!(
            err.to_string().contains("column")
                || err.to_string().contains("missing")
                || err.to_string().contains("empty"),
            "Expected descriptive error, got: {err}"
        );
    }

    #[test]
    fn expand_covering_spec_rejects_missing_columns_after_name() {
        let err = IndexDefinition::expand_covering_spec("prefix=")
            .expect_err("spec missing columns should fail");
        assert!(
            err.to_string().contains("missing column"),
            "Expected 'missing column' error, got: {err}"
        );
    }
}
