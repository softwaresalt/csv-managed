# CLI Contract: CSV-Managed — Baseline SDD Specification

**Branch**: `001-baseline-sdd-spec` | **Date**: 2026-02-13

This document defines the external interface contract for each csv-managed CLI
command. Each entry specifies inputs, outputs, exit codes, and error conditions.

## Global Conventions

| Convention | Value |
|------------|-------|
| Exit code: success | `0` |
| Exit code: user error | `1` |
| Timing output | All commands emit structured timing (start, end, duration) via logger |
| Delimiter auto-detection | `.csv` → comma, `.tsv`/`.tab` → tab |
| Default encoding | UTF-8 (input and output) |
| Stdin sentinel | `-i -` reads from stdin |
| Stdout default | Output goes to stdout when `-o` is omitted |
| Field quoting | All fields quoted in CSV output |

---

## Command: `schema`

### Subcommand: `schema probe`

**Purpose**: Display inferred schema details without writing a file.

| Parameter | Flag | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| input | `-i`, `--input` | Yes | — | CSV file to inspect |
| sample_rows | `--sample-rows` | No | 2000 | Rows to sample (0 = full scan) |
| delimiter | `--delimiter` | No | auto | CSV delimiter character |
| input_encoding | `--input-encoding` | No | utf-8 | Input character encoding |
| mapping | `--mapping` | No | false | Emit column mapping templates |
| overrides | `--override` | No | — | Override inferred types (`name:type`, repeatable) |
| snapshot | `--snapshot` | No | — | Write/validate header+type hash snapshot (JSON) |
| na_behavior | `--na-behavior` | No | empty | How to treat NA placeholders (`empty` or `fill`) |
| na_fill | `--na-fill` | No | "" | Fill value when `--na-behavior=fill` |
| assume_header | `--assume-header` | No | auto | Force header detection (`true` or `false`) |

**Output**: Inference table to stdout (column name, detected type, sample values,
null/placeholder counts, candidate key indicators).

**Error conditions**:
- Input file does not exist → exit 1
- Input file is empty (0 bytes) → graceful report, exit 0
- Encoding not recognized → exit 1

---

### Subcommand: `schema infer`

**Purpose**: Infer schema metadata and optionally persist a YAML schema file.

| Parameter | Flag | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| *(all probe args)* | — | — | — | Inherits all `probe` parameters |
| output | `-o`, `--output`, `-m` | No | — | Destination schema YAML file |
| replace_template | `--replace-template` | No | false | Add empty replace arrays as template |
| preview | `--preview` | No | false | Render to stdout instead of writing file |
| diff | `--diff` | No | — | Show unified diff against existing schema |

**Output**: YAML schema file (if `-o` specified) or stdout preview. Diff output
when `--diff` is specified.

**Error conditions**:
- Cannot write to output path → exit 1
- Diff target file does not exist → exit 1

---

### Subcommand: `schema verify`

**Purpose**: Validate CSV files against a schema definition.

| Parameter | Flag | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| schema | `-m`, `--schema` | Yes | — | Schema YAML file |
| inputs | `-i`, `--input` | Yes | — | CSV files to verify (repeatable) |
| delimiter | `--delimiter` | No | auto | CSV delimiter character |
| input_encoding | `--input-encoding` | No | utf-8 | Input encoding |
| report_invalid | `--report-invalid` | No | summary | Report mode: `summary`, `detail`, optional limit |

**Output**: Verification report to stdout. Summary shows counts per column.
Detail lists individual row/column violations.

**Exit codes**:
- `0` — all files pass verification
- `1` — one or more files have violations

**Error conditions**:
- Schema file does not exist → exit 1
- Header mismatch between CSV and schema → reported as violation
- Input file does not exist → exit 1

---

### Subcommand: `schema columns`

**Purpose**: List column names and types from a schema file.

| Parameter | Flag | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| schema | `-m`, `--schema` | Yes | — | Schema YAML file |

**Output**: Formatted table showing position, column name, data type, and rename
mapping for each column.

---

### Manual `schema` (no subcommand)

**Purpose**: Create a schema YAML from explicit `--column` definitions.

| Parameter | Flag | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| output | `-o`, `--output`, `-m` | Yes | — | Destination schema YAML file |
| columns | `-c`, `--column` | Yes | — | Column definitions (`name:type`, repeatable) |
| replacements | `--replace` | No | — | Value replacement directives (`column=value->replacement`) |

**Output**: YAML schema file written to specified path.

---

## Command: `index`

**Purpose**: Create a B-Tree index file for sort acceleration.

| Parameter | Flag | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| input | `-i`, `--input` | Yes | — | CSV file to index |
| index | `-o`, `--index` | Yes | — | Output index file (.idx) |
| columns | `-C`, `--columns` | No | — | Columns for single ascending index (comma-separated) |
| specs | `--spec` | No | — | Named index specs (`name=col:dir,...`, repeatable) |
| coverings | `--covering` | No | — | Covering specs with direction expansion (repeatable) |
| schema | `-m`, `--schema` | No | — | Schema file for typed comparison |
| limit | `--limit` | No | — | Row limit for prototyping |
| delimiter | `--delimiter` | No | auto | CSV delimiter character |
| input_encoding | `--input-encoding` | No | utf-8 | Input encoding |

**Output**: Binary `.idx` file containing one or more named index variants.

**Error conditions**:
- No columns, specs, or coverings specified → exit 1
- Input file does not exist → exit 1
- Invalid spec syntax → exit 1 with parse error
- Cannot write to index path → exit 1

---

## Command: `process`

**Purpose**: Transform a CSV file using sorting, filtering, projection,
derivations, and schema-driven replacements.

| Parameter | Flag | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| input | `-i`, `--input` | Yes | — | Input CSV file (or `-` for stdin) |
| output | `-o`, `--output` | No | stdout | Output CSV file |
| schema | `-m`, `--schema` | No | — | Schema file for typed operations |
| index | `-x`, `--index` | No | — | Index file for sort acceleration |
| index_variant | `--index-variant` | No | auto | Specific index variant name |
| sort | `--sort` | No | — | Sort directives (`column[:asc\|desc]`, repeatable) |
| columns | `-C`, `--columns` | No | all | Include columns (repeatable) |
| exclude_columns | `--exclude-columns` | No | — | Exclude columns (repeatable) |
| derives | `--derive` | No | — | Derived columns (`name=expression`, repeatable) |
| filters | `--filter` | No | — | Row filters (`column op value`, repeatable, AND) |
| filter_exprs | `--filter-expr` | No | — | Expression filters (repeatable, AND) |
| row_numbers | `--row-numbers` | No | false | Emit 1-based row numbers as first column |
| limit | `--limit` | No | — | Maximum output rows |
| delimiter | `--delimiter` | No | auto | Input delimiter |
| output_delimiter | `--output-delimiter` | No | input | Output delimiter |
| input_encoding | `--input-encoding` | No | utf-8 | Input encoding |
| output_encoding | `--output-encoding` | No | utf-8 | Output encoding |
| boolean_format | `--boolean-format` | No | original | Boolean output: `original`, `true-false`, `one-zero` |
| preview | `--preview` | No | false | Render as formatted table (no CSV output) |
| table | `--table` | No | false | Render as elastic ASCII table |
| apply_mappings | `--apply-mappings` | No | false | Apply schema datatype mappings |
| skip_mappings | `--skip-mappings` | No | false | Skip schema datatype mappings |

**Output**: CSV to file or stdout; formatted table if `--preview` or `--table`.

**Error conditions**:
- Input file does not exist → exit 1
- Referenced column in filter/sort/derive does not exist → exit 1
- Invalid expression syntax → exit 1 with parse error and position
- Index variant not found → exit 1 with available variant list
- No sort match in index → falls back to in-memory sort (not an error)

---

## Command: `append`

**Purpose**: Concatenate multiple CSV files into a single output.

| Parameter | Flag | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| inputs | `-i`, `--input` | Yes | — | Input CSV files (repeatable, min 2) |
| output | `-o`, `--output` | No | stdout | Output CSV file |
| schema | `-m`, `--schema` | No | — | Schema for validation during append |
| delimiter | `--delimiter` | No | auto | CSV delimiter |
| input_encoding | `--input-encoding` | No | utf-8 | Input encoding |
| output_encoding | `--output-encoding` | No | utf-8 | Output encoding |

**Output**: Single CSV file with unified header and all rows.

**Error conditions**:
- Header mismatch between input files → exit 1
- Type violation during schema-driven append → reported and exit 1

---

## Command: `stats`

**Purpose**: Compute summary statistics or frequency counts for CSV columns.

| Parameter | Flag | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| input | `-i`, `--input` | Yes | — | Input CSV file (or `-` for stdin) |
| schema | `-m`, `--schema` | No | — | Schema for typed parsing |
| columns | `-C`, `--columns` | No | numeric/temporal | Target columns (repeatable) |
| filters | `--filter` | No | — | Row filters (repeatable, AND) |
| filter_exprs | `--filter-expr` | No | — | Expression filters (repeatable, AND) |
| delimiter | `--delimiter` | No | auto | CSV delimiter |
| input_encoding | `--input-encoding` | No | utf-8 | Input encoding |
| limit | `--limit` | No | 0 (all) | Maximum rows to scan |
| frequency | `--frequency` | No | false | Emit frequency counts instead of summary |
| top | `--top` | No | 0 (all) | Top-N distinct values per column |

**Output**: Formatted statistics table or frequency table to stdout.

**Metrics (summary mode)**: count, min, max, mean, median, standard deviation.

**Metrics (frequency mode)**: value, count, percentage per distinct value.

---

## Command: `install`

**Purpose**: Install or update the csv-managed binary via `cargo install`.

| Parameter | Flag | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| version | `--version` | No | — | Specific version to install |
| force | `--force` | No | false | Force reinstall |
| locked | `--locked` | No | false | Use `--locked` flag |
| root | `--root` | No | — | Custom install root directory |

**Output**: Delegates to `cargo install csv-managed` with specified flags.

---

## Expression Functions

Available in `--derive` and `--filter-expr` contexts:

### Temporal Functions

| Function | Signature | Description |
|----------|-----------|-------------|
| `date_add` | `date_add(date, days)` | Add days to a date |
| `date_sub` | `date_sub(date, days)` | Subtract days from a date |
| `date_diff_days` | `date_diff_days(date1, date2)` | Integer day difference |
| `date_format` | `date_format(date, fmt)` | Format date as string |
| `datetime_add_seconds` | `datetime_add_seconds(dt, secs)` | Add seconds to datetime |
| `datetime_diff_seconds` | `datetime_diff_seconds(dt1, dt2)` | Seconds difference |
| `datetime_format` | `datetime_format(dt, fmt)` | Format datetime as string |
| `datetime_to_date` | `datetime_to_date(dt)` | Extract date from datetime |
| `datetime_to_time` | `datetime_to_time(dt)` | Extract time from datetime |
| `time_add_seconds` | `time_add_seconds(time, secs)` | Add seconds to time |
| `time_diff_seconds` | `time_diff_seconds(t1, t2)` | Seconds difference |

### String Functions

| Function | Signature | Description |
|----------|-----------|-------------|
| `concat` | `concat(a, b, ...)` | Concatenate values as strings |

### Logic Functions

| Function | Signature | Description |
|----------|-----------|-------------|
| `if` | `if(cond, true_val, false_val)` | Conditional expression |

### Context Variables

| Variable | Description |
|----------|-------------|
| `column_name` | Column value by name |
| `c0`, `c1`, … | Column value by zero-based position |
| `row_number` | Current row index (requires `--row-numbers`) |

### Filter Operators

| Operator | Description |
|----------|-------------|
| `=` | Equals |
| `!=` | Not equals |
| `>` | Greater than |
| `<` | Less than |
| `>=` | Greater than or equal |
| `<=` | Less than or equal |
| `contains` | String contains |
| `startswith` | String starts with |
| `endswith` | String ends with |
