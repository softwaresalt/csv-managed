# Feature Specification: CSV-Managed — Baseline SDD Specification

**Feature Branch**: `001-baseline-sdd-spec`  
**Created**: 2026-02-14  
**Status**: Draft  
**Input**: User description: "Write the spec for the existing csv-managed solution to bring it into alignment with spec driven development practices and for moving forward in future feature development with SDD"

## User Scenarios & Testing *(mandatory)*

### User Story 1 — Schema Discovery & Inference (Priority: P1)

A data engineer receives a large CSV file from an upstream system and needs to understand its structure before loading it into a pipeline. They point csv-managed at the file to automatically detect column names, infer data types, detect whether a header row exists, identify placeholder values (NA, N/A, #N/A, null), and produce a reusable schema definition file.

**Why this priority**: Schema discovery is the foundation for every other operation. Without a schema, typed processing, verification, indexing, and statistics all operate in degraded or untyped mode. This is the first thing a new user does.

**Independent Test**: Can be fully tested by running `schema probe` and `schema infer` against any CSV file and verifying the output schema matches the actual column structure and types.

**Acceptance Scenarios**:

1. **Given** a CSV file with mixed column types, **When** the user runs `schema probe`, **Then** the tool displays an inference table showing each column's name, detected type, sample values, and null/placeholder counts — without writing any file.
2. **Given** a CSV file, **When** the user runs `schema infer -o output-schema.yml`, **Then** a YAML schema file is written containing column order, types, optional renames, and a replace template if requested.
3. **Given** a CSV file without a header row, **When** the user runs `schema infer`, **Then** the tool detects the absence of headers, assigns synthetic names (`field_0`, `field_1`, …), and produces a valid schema.
4. **Given** a CSV file with NA-style placeholders, **When** the user runs `schema probe --na-behavior fill --na-fill "MISSING"`, **Then** placeholder values are normalized to the specified fill value in the inference output.
5. **Given** an existing schema and a modified CSV, **When** the user runs `schema infer --diff existing-schema.yml`, **Then** a unified diff is displayed showing column additions, removals, and type changes.
6. **Given** a CSV file, **When** the user runs `schema infer --snapshot snapshot.json`, **Then** a snapshot is written containing a SHA-256 hash of the header layout and type assignments, enabling future regression detection.

---

### User Story 2 — Data Transformation & Processing (Priority: P1)

A data scientist needs to filter, sort, project, derive new columns, and transform a CSV file before feeding it to a training pipeline. They use the `process` command to apply a chain of operations in a single pass: schema-driven type mappings, value replacements, row filters, column selection/exclusion, derived expressions, sorting (optionally accelerated by a prebuilt index), and output in CSV or table format.

**Why this priority**: Processing is the core value proposition — transforming raw data into pipeline-ready output. This is the command users invoke most frequently.

**Independent Test**: Can be fully tested by running `process` with filters, derives, sort, and column selection against a known CSV and verifying the output matches expected rows and values.

**Acceptance Scenarios**:

1. **Given** a CSV file and a schema, **When** the user runs `process` with `--filter "amount >= 100"`, **Then** only rows where the typed value of `amount` is at least 100 appear in output.
2. **Given** a CSV file, **When** the user runs `process` with `--derive "total_with_tax=amount*1.0825"`, **Then** a new column `total_with_tax` appears in the output with correctly computed values.
3. **Given** a CSV file and an index file, **When** the user runs `process` with `--sort order_date:asc` and the index contains a matching variant, **Then** output is sorted using the index (not an in-memory sort) and performance is proportional to I/O rather than row count.
4. **Given** a CSV file, **When** the user runs `process` with `--columns name,email --exclude-columns internal_id`, **Then** only the specified columns appear, excluding the named exclusions.
5. **Given** a CSV file with boolean values in various formats (yes/no, 1/0, true/false), **When** the user runs `process` with `--boolean-format true-false`, **Then** all boolean columns are normalized to `true`/`false` in output.
6. **Given** a CSV file, **When** the user runs `process` with `--preview --limit 15`, **Then** the first 15 rows are rendered as a formatted table on stdout, and no output file is written.
7. **Given** a schema with value replacements (e.g., "M" → "Male"), **When** the user runs `process` with that schema, **Then** replacement mappings are applied to matching cell values in the output.
8. **Given** a schema with datatype mappings (e.g., rounding a decimal to 2 places), **When** the user runs `process` with `--apply-mappings`, **Then** mapping transformations are applied before final type parsing.

---

### User Story 3 — Schema Verification (Priority: P1)

A data engineer needs to validate that incoming CSV files conform to an expected schema before loading them into a data warehouse. They run the `schema verify` command against one or more input files and receive a report of any rows or cells that violate the schema's type or value constraints.

**Why this priority**: Verification is the gatekeeping function — preventing bad data from entering downstream systems. In production pipelines, this is used as a pre-load validation step.

**Independent Test**: Can be fully tested by running `schema verify` against a CSV file with known invalid rows and confirming the tool reports the correct violations.

**Acceptance Scenarios**:

1. **Given** a schema and a CSV file where some cells contain values that do not match the declared type, **When** the user runs `schema verify`, **Then** a summary report shows the count of invalid cells per column.
2. **Given** a schema and a CSV file with invalid rows, **When** the user runs `schema verify --report-invalid detail`, **Then** a detail report lists each invalid row number, column, raw value, and expected type.
3. **Given** a schema and a CSV file with an incompatible header (missing or extra columns), **When** the user runs `schema verify`, **Then** the tool reports the header mismatch and exits with a non-zero status.
4. **Given** a schema and multiple CSV files, **When** the user runs `schema verify -i file1.csv -i file2.csv`, **Then** each file is verified independently and results are reported per file.
5. **Given** a schema and a CSV with thousands of invalid rows, **When** the user uses `--report-invalid detail 50`, **Then** the detail report is capped at 50 entries with a note indicating more exist.

---

### User Story 4 — B-Tree Indexing for Sort Acceleration (Priority: P2)

A data engineer frequently sorts large CSV files on the same columns. They build a multi-variant index file once, then reference it in subsequent `process` runs to avoid expensive in-memory sorts. The index stores byte offsets keyed by column values and supports multiple named variants with different column/direction combinations.

**Why this priority**: Indexing is a performance multiplier for repeated sort operations on large files. It's not required for basic use but becomes essential for production-scale workflows.

**Independent Test**: Can be fully tested by building an index, then running `process` with a matching sort and verifying the output order matches the index definition.

**Acceptance Scenarios**:

1. **Given** a CSV file, **When** the user runs `index` with `--spec default=order_date:asc,customer_id:asc`, **Then** a binary index file is written containing a named variant "default" with ascending sort on both columns.
2. **Given** an index build command with multiple `--spec` flags, **When** the index is built, **Then** the resulting `.idx` file contains all named variants and each can be used independently.
3. **Given** a `--covering` spec like `geo=date:asc|desc,customer:asc`, **When** the index is built, **Then** the tool generates all direction/prefix permutations as separate variants under the "geo" family.
4. **Given** a `process` command with `--sort` that partially matches an index variant, **When** the tool selects an index variant, **Then** it chooses the variant with the longest matching column prefix.
5. **Given** a `process` command with `--index-variant specific_name`, **When** that variant does not exist in the index file, **Then** the tool reports a clear error identifying the missing variant.

---

### User Story 5 — Summary Statistics & Frequency Analysis (Priority: P2)

A data scientist wants to quickly profile a CSV file's numeric and temporal columns — computing count, min, max, mean, median, and standard deviation — or generate frequency counts (top-N distinct values) for categorical columns.

**Why this priority**: Statistics and frequency counts are essential for data profiling and quality assessment. They enable quick understanding of data distributions before building more complex transformations.

**Independent Test**: Can be fully tested by running `stats` and `stats --frequency` against a known CSV and verifying computed metrics match expected values.

**Acceptance Scenarios**:

1. **Given** a CSV file with numeric columns, **When** the user runs `stats`, **Then** the tool outputs count, min, max, mean, median, and standard deviation for each numeric column.
2. **Given** a CSV file with temporal columns (date, datetime, time), **When** the user runs `stats`, **Then** the tool outputs meaningful temporal metrics (earliest, latest, count).
3. **Given** a CSV file, **When** the user runs `stats --frequency --top 10`, **Then** the top 10 distinct values per column are displayed with counts and percentages.
4. **Given** a CSV file with schema, **When** the user runs `stats --filter "region = US"`, **Then** statistics are computed only for rows matching the filter.
5. **Given** a CSV file with decimal/currency columns, **When** the user runs `stats`, **Then** precision and scale are preserved in the statistical output.

---

### User Story 6 — Multi-File Append (Priority: P2)

A data engineer needs to concatenate multiple CSV files that share the same schema into a single output file. The tool validates header consistency across all inputs and optionally enforces schema compliance during the union.

**Why this priority**: Multi-file append is a common ETL pattern when data arrives in batches (daily files, partitioned exports). Header consistency validation prevents silent data corruption.

**Independent Test**: Can be fully tested by appending two or more CSV files and verifying the output contains all rows with correct headers.

**Acceptance Scenarios**:

1. **Given** multiple CSV files with identical headers, **When** the user runs `append -i a.csv -i b.csv -o combined.csv`, **Then** a single output file contains all rows from both inputs with the header written once.
2. **Given** multiple CSV files with mismatched headers, **When** the user runs `append`, **Then** the tool reports a header mismatch error and does not produce output.
3. **Given** multiple CSV files and a schema, **When** the user runs `append` with `--schema`, **Then** each row is validated against the schema during the union and type violations are reported.

---

### User Story 7 — Streaming Pipeline Support (Priority: P2)

A data engineer chains multiple csv-managed commands together using shell pipes, reading from stdin and writing to stdout, to build multi-stage data transformation pipelines without intermediate files.

**Why this priority**: Pipeline composition is a key Unix-philosophy capability that enables complex workflows without disk I/O overhead for intermediate results.

**Independent Test**: Can be fully tested by piping the output of one command into another (e.g., `process | stats`) and verifying the final output is correct.

**Acceptance Scenarios**:

1. **Given** a CSV file, **When** the user pipes `process` output into `stats` using `-i -` for stdin, **Then** statistics are computed on the transformed output without writing an intermediate file.
2. **Given** a CSV file with non-UTF-8 encoding, **When** the user runs `process` with `--input-encoding windows-1252 --output-encoding utf-8`, **Then** the output is correctly transcoded.
3. **Given** a process command with `--preview`, **When** the user pipes it downstream, **Then** the preview mode writes formatted table output to stdout (not CSV), making it unsuitable for piping and the tool behaves accordingly.

---

### User Story 8 — Expression Engine for Derived Logic & Filtering (Priority: P3)

A data scientist needs to compute complex derived columns or apply sophisticated filter conditions that go beyond simple comparisons. They use the expression engine to write formulas involving arithmetic, string operations, conditional logic, and temporal helper functions.

**Why this priority**: The expression engine powers advanced use cases (date arithmetic, conditional flags, multi-column calculations) that differentiate csv-managed from simpler CSV tools.

**Independent Test**: Can be fully tested by running `process` with `--derive` and `--filter-expr` arguments and verifying computed values and filtered results.

**Acceptance Scenarios**:

1. **Given** a CSV with date columns, **When** the user derives `ship_lag=date_diff_days(shipped_at,ordered_at)`, **Then** the output column contains the integer day difference for each row.
2. **Given** a CSV with numeric columns, **When** the user applies `--filter-expr "amount > 1000 && status == \"shipped\""`, **Then** only rows matching both conditions appear.
3. **Given** a CSV, **When** the user derives `channel_tag=concat(channel,"-",region)`, **Then** the output column contains the concatenated string.
4. **Given** `--row-numbers` is enabled, **When** the user derives `row_idx=row_number`, **Then** each row's sequential index is available in the expression context.
5. **Given** a CSV, **When** the user uses positional aliases `c0`, `c1` in expressions, **Then** columns are resolved by their zero-based position.

---

### User Story 9 — Schema Column Listing (Priority: P3)

A user wants to quickly view the columns and types defined in an existing schema file without opening it in a text editor. They run `schema columns` to see a formatted table of column positions, names, types, and any renames.

**Why this priority**: A convenience feature that improves workflow efficiency when working with schemas across multiple terminal sessions.

**Independent Test**: Can be fully tested by running `schema columns -m schema.yml` and verifying the table output matches the schema definition.

**Acceptance Scenarios**:

1. **Given** a schema YAML file, **When** the user runs `schema columns -m schema.yml`, **Then** a formatted table shows position, column name, data type, and renamed output name for each column.
2. **Given** a schema YAML file containing columns with renames and datatype mappings, **When** the user runs `schema columns -m schema.yml`, **Then** the table displays both original and renamed column names alongside their declared data types.

---

### User Story 10 — Self-Install (Priority: P3)

A user wants to install or update csv-managed using a convenient wrapper command rather than remembering the full `cargo install` incantation.

**Why this priority**: A quality-of-life feature that simplifies installation for users who already have the tool.

**Independent Test**: Can be fully tested by running `install --version X.Y.Z` and verifying the correct cargo command is executed.

**Acceptance Scenarios**:

1. **Given** csv-managed is already built, **When** the user runs `install --locked`, **Then** the tool executes `cargo install csv-managed --locked` using the current source.
2. **Given** a specific version is needed, **When** the user runs `install --version 1.0.2`, **Then** that exact version is installed from crates.io.

---

### Edge Cases

- What happens when a CSV file is empty (zero rows)? The tool should report the empty state gracefully without crashing.
- What happens when a CSV file has only a header row and no data rows? Statistics should report zero counts; verification should succeed.
- What happens when a filter expression references a column that does not exist? The tool should report a clear error identifying the unknown column.
- What happens when a derive expression has a syntax error? The tool should report the parse error with the expression text and position.
- What happens when an index file was built with a different index format version? The tool should detect format version incompatibility and suggest rebuilding the index.
- What happens when stdin provides no data (empty pipe)? The tool should detect the empty stream and report it.
- What happens when a decimal value exceeds the maximum precision (28 digits)? The tool should report a precision overflow error.
- What happens when a schema defines a column rename but the CSV header uses the original name? The tool should map the original header name to the renamed output name transparently.
- What happens when multiple `--filter` flags are specified? They should be combined with AND semantics.
- What happens when `--sort` is specified but no index matches and the file is very large? The tool should fall back to in-memory sort and complete correctly; users should be aware this buffers all rows in memory and should prefer building an index for large files.

## Requirements *(mandatory)*

### Functional Requirements

#### Schema Management

- **FR-001**: System MUST infer column names and data types from a CSV file by sampling a configurable number of rows (default: 2000; 0 for full scan).
- **FR-002**: System MUST detect the presence or absence of a header row and assign synthetic names (`field_0`, `field_1`, …) when no header is detected.
- **FR-003**: System MUST support an `--assume-header` flag to override automatic header detection.
- **FR-004**: System MUST persist inferred schemas as YAML files (`*-schema.yml`) containing column order, types, optional renames, datatype mappings, and value replacements.
- **FR-005**: System MUST support schema probing (read-only inspection) that displays an inference table without writing a file.
- **FR-006**: System MUST support unified diff output between an inferred schema and an existing schema file.
- **FR-007**: System MUST support snapshots that capture a SHA-256 hash of header layout and type assignments for regression detection.
- **FR-008**: System MUST support overriding inferred types for specific columns via `--override name:type`.
- **FR-009**: System MUST detect and normalize NA-style placeholders (NA, N/A, #N/A, #NA, null, none) with configurable behavior: `empty` (treat as empty string, the default) or `fill` (replace with a user-specified value via `--na-fill`).
- **FR-010**: System MUST support manual schema creation from explicit `--column name:type` definitions.
- **FR-011**: System MUST emit mapping scaffolds and snake_case naming suggestions via `--mapping`.

#### Data Types

- **FR-012**: System MUST support the following column types: String, Integer (64-bit signed), Float (double precision), Boolean, Date, DateTime, Time, GUID, Decimal (fixed precision/scale ≤28), and Currency (2 or 4 decimal places).
- **FR-013**: System MUST parse boolean values from multiple input formats: true/false, yes/no, 1/0, t/f, y/n.
- **FR-014**: System MUST parse dates from common formats (YYYY-MM-DD, MM/DD/YYYY) and canonicalize to `YYYY-MM-DD`.
- **FR-015**: System MUST parse currency values with optional symbols ($, €, £, ¥), thousands separators, and negative formats including parentheses notation.
- **FR-016**: System MUST support decimal types with configurable precision and scale, and enforce rounding strategies: truncate (discard excess digits) and round-half-up (standard arithmetic rounding).

#### Processing & Transformation

- **FR-017**: System MUST support row-level filtering using typed comparisons with operators: =, !=, >, <, >=, <=, contains, startswith, endswith.
- **FR-018**: System MUST support expression-based filtering using a full expression language with boolean logic (AND, OR, nested if).
- **FR-019**: System MUST support column projection (include list) and column exclusion.
- **FR-020**: System MUST support derived columns computed from expressions, including arithmetic, string concatenation, conditional logic, and temporal helper functions.
- **FR-021**: System MUST support sorting by one or more columns with per-column ascending/descending direction.
- **FR-022**: System MUST apply schema-defined datatype mappings (parse, round, trim, case) in order before replacements and final type parsing.
- **FR-023**: System MUST apply schema-defined value replacements (raw value → normalized value) during processing.
- **FR-024**: System MUST support row number injection as an optional first column.
- **FR-025**: System MUST support configurable boolean output formats (original, true-false, one-zero).
- **FR-026**: System MUST support row limit to restrict the number of output rows.
- **FR-027**: System MUST support preview mode (`--preview`) that renders a fixed-width quick-view table on stdout, truncating wide columns for at-a-glance inspection.
- **FR-028**: System MUST support table mode (`--table`) that renders an elastic-width ASCII table on stdout with dynamically sized columns to display full cell values.

#### Expression Engine

- **FR-029**: System MUST provide temporal helper functions: date_add, date_sub, date_diff_days, date_format, datetime_add_seconds, datetime_diff_seconds, datetime_format, datetime_to_date, datetime_to_time, time_add_seconds, time_diff_seconds.
- **FR-030**: System MUST provide string functions: concat.
- **FR-031**: System MUST provide conditional logic: if(condition, true_value, false_value).
- **FR-032**: System MUST expose columns by name and by positional alias (c0, c1, …) in expression contexts.
- **FR-033**: System MUST expose `row_number` in expression contexts when `--row-numbers` is enabled.

#### Indexing

- **FR-034**: System MUST build B-Tree index files storing byte offsets keyed by concatenated column values.
- **FR-035**: System MUST support multiple named index variants within a single index file, each with different column sets and sort directions.
- **FR-036**: System MUST support covering index expansion from a concise specification pattern, generating all direction/prefix permutations.
- **FR-037**: System MUST select the best-matching index variant by finding the longest column prefix that matches a requested sort.
- **FR-038**: System MUST support pinning a specific index variant by name via `--index-variant`.
- **FR-039**: System MUST serialize index files in a versioned binary format and detect version incompatibility.
- **FR-040**: System MUST perform index-accelerated sorting as a streaming operation, reading rows by seek offset without buffering the entire file in memory, enabling sort of arbitrarily large files.

#### Verification

- **FR-041**: System MUST validate every cell in a CSV file against the declared schema type.
- **FR-042**: System MUST support tiered invalid reporting: summary (counts per column), detail (individual rows/cells), and configurable limits.
- **FR-043**: System MUST detect and report header mismatches between a CSV file and its schema.
- **FR-044**: System MUST support verifying multiple input files against a single schema in one invocation.

#### Statistics

- **FR-045**: System MUST compute count, min, max, mean, median, and standard deviation for numeric and temporal columns.
- **FR-046**: System MUST support frequency analysis showing top-N distinct values per column with counts and percentages.
- **FR-047**: System MUST support filtering rows before computing statistics.

#### Append

- **FR-048**: System MUST concatenate multiple CSV files into a single output, writing the header once.
- **FR-049**: System MUST validate header consistency across all input files before appending.
- **FR-050**: System MUST support schema-driven validation during append.

#### I/O & Encoding

- **FR-051**: System MUST auto-detect delimiter based on file extension (.csv → comma, .tsv → tab) with manual override support for comma, tab, pipe, semicolon, and any single ASCII character.
- **FR-052**: System MUST support independent input and output character encodings (defaulting to UTF-8).
- **FR-053**: System MUST support reading from stdin (`-i -`) and writing to stdout for pipeline composition.
- **FR-054**: System MUST quote all fields in CSV output to ensure round-trip safety.

#### Installation

- **FR-055**: System MUST provide a self-install command wrapping `cargo install` with options for version, force, locked, and custom root.

#### Observability

- **FR-056**: System MUST emit structured timing information (start time, end time, duration in seconds) for every operation upon completion.
- **FR-057**: System MUST support configurable log verbosity levels, allowing users to increase detail for diagnostics (e.g., inference voting, index selection, mapping application) without modifying code.
- **FR-058**: System MUST log operation outcomes (success or failure with error context) using the same structured format as timing output.
- **FR-059**: System MUST exit with code 0 on successful completion and a non-zero exit code on any error (parse failure, verification violation, missing file, invalid expression), enabling reliable use in automated pipelines and shell scripts.

### Key Entities

- **Schema**: Defines the expected structure of a CSV file — column order, names, data types, optional renames, datatype mapping chains, and value replacement rules. Persisted as YAML.
- **Column Type**: One of 10 supported types (String, Integer, Float, Boolean, Date, DateTime, Time, GUID, Decimal, Currency) that determines parsing, comparison, and output formatting rules.
- **Value**: A typed cell value parsed from a raw CSV field according to its column type. Supports null-safe ordering and expression evaluation.
- **Index**: A binary file containing one or more named variants. Each variant maps sorted composite key values to byte offsets in the source CSV for O(1) seek access.
- **Index Variant**: A named sort configuration within an index file, defined by column names and per-column sort directions (ascending/descending).
- **Datatype Mapping**: An ordered transformation chain applied to raw cell values before final type parsing — including parse, round, trim, and case operations.
- **Value Replacement**: A mapping from a raw cell value to a normalized output value, applied per-column after datatype mappings but before final type parsing.
- **Snapshot**: A regression-detection artifact containing a SHA-256 hash of header layout and type assignments, enabling future regression detection.
- **Filter Condition**: A typed comparison rule (column, operator, value) used to include or exclude rows during processing.
- **Derived Column**: A new output column computed from an expression involving existing columns, constants, and built-in functions.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Users can discover the schema of an unknown CSV file in under 30 seconds for files up to 10 GB, receiving a complete type-annotated column listing.
- **SC-002**: Users can filter, sort (with index), project, and derive columns from arbitrarily large CSV files in a single streaming pass, bounded only by disk I/O rather than available memory.
- **SC-003**: Users can validate a CSV file against a schema and receive a clear, actionable report of all type violations categorized by column.
- **SC-004**: Users can build an index once and reuse it across multiple process runs, reducing sort time proportionally to avoiding full-file scans.
- **SC-005**: Users can chain multiple csv-managed commands in a shell pipeline, reading from stdin and writing to stdout, without requiring intermediate files.
- **SC-006**: Users can profile a CSV file's numeric distributions (count, min, max, mean, median, std dev) or categorical distributions (frequency counts) in a single command.
- **SC-007**: Users can concatenate multiple same-schema CSV files with header consistency validation, preventing silent column misalignment.
- **SC-008**: All existing automated tests pass, all code meets formatting standards, and no compiler or linter warnings are produced.
- **SC-009**: The tool handles CSV files with non-UTF-8 encodings correctly when the appropriate `--input-encoding` flag is provided.
- **SC-010**: All error messages include contextual information about the operation that failed, the file being processed, and the specific row/column when applicable.

## Clarifications

### Session 2026-02-14

- Q: Should this baseline spec cover only implemented features (v1.0.2) or the full roadmap? → A: Current implementation only; future features will receive their own specs via `/speckit.specify`.
- Q: Should the implemented-but-CLI-disabled join subcommand be documented in this spec? → A: Exclude; note as dormant pending v2.5.0 redesign.
- Q: Should operation timing output and configurable log levels be formal requirements? → A: Yes, add requirements for both operation timing and configurable log levels.
- Q: What is the realistic file size upper bound for the tool? → A: Indexed sort should be streaming (no in-memory buffering), enabling arbitrarily large files. Only non-indexed sort and median require buffering and are memory-bound.
- Q: Should the spec define explicit exit code behavior? → A: Yes, add requirement for 0 on success and non-zero on any error.
- Q: Should the `--skip-mappings` and `--output-delimiter` flags have their own functional requirements? → A: No. These flags invert or extend existing requirements (FR-022 for mappings, FR-051 for delimiters) and are documented in the CLI contract. They do not introduce new behavior categories.

## Assumptions

- This specification documents the feature set as of v1.0.2. Planned features from the roadmap (v1.1.0 through v6.0.0) are out of scope and will each receive their own specification when entering development.
- The tool is designed as a single-user CLI utility, not a multi-tenant service. Concurrency is not a design requirement.
- Input files are expected to be well-formed CSV/TSV with consistent row lengths. The tool reports but does not attempt to repair structurally malformed files.
- The `join` subcommand has a complete implementation in code but is intentionally disabled in the CLI. It is excluded from this baseline specification and will be re-introduced with streaming enhancements in a future v2.5.0 spec.
- Schema files follow the `*-schema.yml` naming convention by default, though the user may specify any path.
- Index files follow the `.idx` extension convention.
- The tool prioritizes streaming/sequential processing to minimize memory footprint. Filtering, projection, derivation, verification, indexed sort, and append all operate in streaming mode. Only non-indexed sort and median computation buffer data in memory and are therefore bounded by available system memory.
