# Data Model: CSV-Managed — Baseline SDD Specification

**Branch**: `001-baseline-sdd-spec` | **Date**: 2026-02-13

## Entity Relationship Overview

```text
Schema 1──* ColumnMeta 1──* ValueReplacement
                │         1──* DatatypeMapping
                │
                └── ColumnType ──?── DecimalSpec
                         │
                         ▼
                       Value ◀── CurrencyValue
                         │   ◀── FixedDecimalValue
                         ▼
                   ComparableValue (null-aware ordering)
                         │
                         ▼
CsvIndex 1──* IndexVariant ──* IndexDefinition
                │
                └── BTreeMap<Vec<DirectionalComparableValue>, Vec<u64>>
                         │
                         └── byte offsets into source CSV
```

## Core Entities

### Schema

Defines the expected structure of a CSV file. Canonical input for typed
processing, verification, and statistics.

| Field | Type | Description |
|-------|------|-------------|
| `columns` | `Vec<ColumnMeta>` | Ordered list of column definitions |
| `schema_version` | `Option<String>` | Optional version identifier |
| `has_headers` | `bool` | Whether the source CSV has a header row |

**Persistence**: YAML file (`*-schema.yml`)

**Validation rules**:
- Column names must be non-empty and unique within a schema
- At least one column must be defined
- `schema_version` follows semver when present

**State transitions**: None — schemas are immutable once created. Changes
produce a new schema file or diff output.

### ColumnMeta

Describes a single column within a schema, including its type, optional rename,
and transformation rules.

| Field | Type | Serialized As | Description |
|-------|------|---------------|-------------|
| `name` | `String` | `name` | Original column name from CSV header |
| `datatype` | `ColumnType` | `datatype` | Declared or inferred data type |
| `rename` | `Option<String>` | `name_mapping` | Optional output column name |
| `value_replacements` | `Vec<ValueReplacement>` | `replace` | Value substitution rules |
| `datatype_mappings` | `Vec<DatatypeMapping>` | `datatype_mappings` | Ordered type conversion chain |

**Validation rules**:
- `name` must match a header in the source CSV (or mapped via alias)
- `rename` must be unique across all columns if set
- `value_replacements` are applied in order after `datatype_mappings`

### ColumnType

Enumeration of the 10 supported data types. Determines parsing, comparison,
output formatting, and statistical computation rules.

| Variant | Rust Type | Description |
|---------|-----------|-------------|
| `String` | — | Free-text, universal fallback |
| `Integer` | — | 64-bit signed integer |
| `Float` | — | Double-precision floating point |
| `Boolean` | — | Parsed from: true/false, yes/no, 1/0, t/f, y/n |
| `Date` | — | Calendar date, canonicalized to `YYYY-MM-DD` |
| `DateTime` | — | Date + time, supports multiple input formats |
| `Time` | — | Time of day (`HH:MM:SS` or `HH:MM`) |
| `Guid` | — | UUID v4 string |
| `Currency` | — | Decimal with currency symbols, 2 or 4 decimal places |
| `Decimal(DecimalSpec)` | — | Fixed precision/scale decimal (max precision 28) |

**Type inference priority** (most specific to least):
Currency → Decimal → Float → Integer → DateTime → Date → Time →
GUID → Boolean → String

### DecimalSpec

Configuration for fixed-precision decimal columns.

| Field | Type | Description |
|-------|------|-------------|
| `precision` | `u32` | Total digit count (max 28) |
| `scale` | `u32` | Digits after decimal point |

**Validation rules**:
- `precision` ≤ 28
- `scale` ≤ `precision`

### Value

A typed cell value parsed from a raw CSV field. Implements `Eq`, `Ord`,
`Serialize`, `Deserialize` for use in sorting, indexing, and expression
evaluation.

| Variant | Inner Type | Description |
|---------|-----------|-------------|
| `String` | `String` | Raw text value |
| `Integer` | `i64` | Parsed integer |
| `Float` | `f64` | Parsed float (custom Ord via total_cmp) |
| `Boolean` | `bool` | Parsed boolean |
| `Date` | `chrono::NaiveDate` | Parsed calendar date |
| `DateTime` | `chrono::NaiveDateTime` | Parsed date-time |
| `Time` | `chrono::NaiveTime` | Parsed time |
| `Guid` | `uuid::Uuid` | Parsed UUID |
| `Decimal` | `FixedDecimalValue` | Precision-controlled decimal |
| `Currency` | `CurrencyValue` | Currency-formatted decimal |

**Cross-type ordering**: Uses a discriminant index so values of different types
have a deterministic, stable sort order.

### ComparableValue

Newtype wrapper `ComparableValue(pub Option<Value>)` enabling null-aware
ordering for index keys. `None` sorts before all `Some` values.

### CurrencyValue

Wraps `rust_decimal::Decimal` with currency-specific parsing rules.

| Field | Type | Description |
|-------|------|-------------|
| `value` | `Decimal` | Numeric amount |
| `scale` | `u32` | Allowed: 2 or 4 decimal places |

**Parsing**: Strips currency symbols (`$`, `€`, `£`, `¥`), thousands
separators, and handles parenthesized negative notation `(1,234.56)` → `-1234.56`.

### FixedDecimalValue

Wraps `rust_decimal::Decimal` with validated precision and scale.

| Field | Type | Description |
|-------|------|-------------|
| `value` | `Decimal` | Numeric amount |
| `precision` | `u32` | Total digits |
| `scale` | `u32` | Fractional digits |

**Rounding strategies**: Truncate, round-half-up.

### ValueReplacement

Maps raw cell values to normalized output values. Applied per-column after
datatype mappings but before final type parsing.

| Field | Type | Description |
|-------|------|-------------|
| `from` | `String` | Raw input value to match (exact) |
| `to` | `String` | Replacement output value |

### DatatypeMapping

An ordered transformation step applied to raw cell values before final type
parsing. Supports chaining multiple transformations.

| Field | Type | Description |
|-------|------|-------------|
| `from` | `ColumnType` | Source data type |
| `to` | `ColumnType` | Target data type |
| `strategy` | `Option<String>` | Conversion strategy (e.g., "round", "truncate") |
| `options` | `BTreeMap<String, Value>` | Additional parameters (e.g., scale, format) |

### InferenceStats

Holds intermediate results from schema inference sampling.

| Field | Type | Description |
|-------|------|-------------|
| sample_values | per-column samples | Representative values for type voting |
| row_count | `usize` | Total rows sampled |
| decode_errors | `usize` | Encoding failures encountered |
| column_summaries | per-column stats | Type vote counts, null counts |
| placeholder_summaries | per-column | NA/N/A/null placeholder counts |

## Index Entities

### CsvIndex

Top-level index container. Serialized as a single binary file (`.idx`).

| Field | Type | Description |
|-------|------|-------------|
| `version` | `String` | Format version (currently `"v2"`) |
| `headers` | `Vec<String>` | Source CSV headers at build time |
| `variants` | `Vec<IndexVariant>` | Named sort configurations |
| `row_count` | `usize` | Total rows indexed |

**Persistence**: Binary via `bincode` v2 serialization.

**Version compatibility**: Loader checks version string; mismatch produces an
error advising rebuild.

### IndexVariant

A single sorted index configuration mapping composite key values to source
file byte offsets.

| Field | Type | Description |
|-------|------|-------------|
| `name` | `String` | Variant identifier (e.g., `"default"`, `"geo_date_asc_customer_asc"`) |
| `columns` | `Vec<String>` | Column names in sort order |
| `directions` | `Vec<SortDirection>` | Per-column `Asc` or `Desc` |
| `column_types` | `Vec<ColumnType>` | Per-column types for typed comparison |
| `entries` | `BTreeMap<Vec<DirectionalComparableValue>, Vec<u64>>` | Sorted keys → byte offsets |

### IndexDefinition

Input specification for building an index variant.

| Field | Type | Description |
|-------|------|-------------|
| `name` | `Option<String>` | Optional variant name |
| `columns` | `Vec<String>` | Columns to index |
| `directions` | `Vec<SortDirection>` | Per-column sort direction |

**Parsing**: From `--spec` format: `name=col1:asc,col2:desc` or unnamed
`col1:asc,col2:desc`.

**Covering expansion**: `--covering geo=date:asc|desc,customer:asc` generates
all direction/prefix permutations as separate variants.

### SortDirection

| Variant | Description |
|---------|-------------|
| `Asc` | Ascending sort order |
| `Desc` | Descending sort order |

## Processing Pipeline Order

The `process` command applies transformations in this fixed order:

```text
1. Read CSV headers
2. Resolve schema (if provided)
3. Apply column renames/aliases
4. Determine sort strategy (index lookup or in-memory fallback)
5. For each row (streaming or index-ordered):
   a. Apply datatype mappings (if --apply-mappings or schema has mappings)
   b. Apply value replacements (from schema)
   c. Parse typed values
   d. Evaluate row-level filters (--filter)
   e. Evaluate expression filters (--filter-expr)
   f. Compute derived columns (--derive)
   g. Apply boolean format normalization
   h. Apply column projection/exclusion
   i. Inject row number (if --row-numbers)
   j. Emit row (CSV, preview table, or elastic table)
6. Apply row limit (--limit)
7. Write output (file, stdout, or preview)
```

## Verification Pipeline Order

```text
1. Load schema
2. For each input file:
   a. Read CSV headers
   b. Compare headers against schema columns (report mismatches)
   c. For each row:
      i.  Parse each cell against declared column type
      ii. Record failures (column, row number, raw value, expected type)
   d. Emit report (summary counts or detail rows, optionally capped)
```

## Statistics Pipeline Order

```text
1. Load schema (optional)
2. Read CSV headers
3. Determine target columns (numeric/temporal or --columns selection)
4. For each row (streaming, respecting --limit):
   a. Evaluate filters (if any)
   b. Parse values for target columns
   c. Accumulate into per-column accumulators
5. Compute metrics: count, min, max, mean, median, stddev
6. Emit formatted statistics table
```
