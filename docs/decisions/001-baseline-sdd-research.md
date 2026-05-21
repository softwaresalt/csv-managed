# Research: CSV-Managed — Baseline SDD Specification

**Branch**: `001-baseline-sdd-spec` | **Date**: 2026-02-13
**Status**: Complete — all unknowns resolved

## Research Tasks

### 1. Schema inference algorithm and type resolution

**Decision**: The existing inference engine samples up to N rows (default 2,000;
0 = full scan), performs per-column type voting across 10 candidate types, and
selects the narrowest type that accommodates all sampled values. Tie-breaking
favors specificity: Currency > Decimal > Float > Integer > DateTime > Date >
Time > GUID > Boolean > String (String is the universal fallback).

**Rationale**: Voting-based inference scales linearly with sample size and avoids
single-row anomalies skewing the result. The type priority order ensures the most
informative type wins when multiple types parse successfully.

**Alternatives considered**:
- Full-file scan by default: rejected for performance — hundreds-of-GB files
  would take minutes for inference alone.
- Machine-learning-based inference: rejected for complexity and non-determinism.

### 2. Index binary format and versioning strategy

**Decision**: The index uses `bincode` v2 serialization with a format version
field (`v2`). The serialized payload contains: version string, source headers,
a vector of `IndexVariant` structs (each with columns, directions, column types,
and a `BTreeMap<Vec<DirectionalComparableValue>, Vec<u64>>` mapping composite
keys to byte offsets), and total row count.

**Rationale**: `bincode` provides compact, fast serialization. The version field
enables forward-compatible detection — if a newer format is loaded by an older
binary, the version mismatch is reported and the user is advised to rebuild.

**Alternatives considered**:
- Custom binary format with magic bytes: rejected for maintenance cost.
- SQLite-based index: rejected for dependency weight and deployment complexity.
- Protobuf: rejected for schema-management overhead on a single-binary CLI tool.

### 3. Expression engine integration patterns

**Decision**: The expression engine wraps the `evalexpr` crate, registering
custom temporal functions (`date_add`, `date_sub`, `date_diff_days`,
`date_format`, `datetime_add_seconds`, `datetime_diff_seconds`,
`datetime_format`, `datetime_to_date`, `datetime_to_time`, `time_add_seconds`,
`time_diff_seconds`) and string functions (`concat`). Row context is injected
per-row by populating an `evalexpr::HashMapContext` with column values mapped
to their names and positional aliases (`c0`, `c1`, …).

**Rationale**: `evalexpr` provides a safe, sandboxed expression evaluator with
no filesystem or network access, suitable for user-provided formulas. Custom
function registration extends it for domain-specific temporal operations.

**Alternatives considered**:
- Lua/Rhai embedded scripting: rejected for security surface and binary size.
- Custom parser: rejected for development cost and diminishing returns.

### 4. Encoding transcoding approach

**Decision**: Uses `encoding_rs` + `encoding_rs_io` for streaming transcoding.
Input encoding is specified via `--input-encoding` (defaults to UTF-8). Output
encoding via `--output-encoding` (defaults to UTF-8). The transcoding wraps the
raw reader/writer in a `DecodeReaderBytes` adapter, enabling transparent
conversion without buffering the entire file.

**Rationale**: `encoding_rs` is the Mozilla-maintained encoding library used in
Firefox, providing correct, fast transcoding for all WHATWG-specified encodings.
Streaming adapter avoids memory overhead.

**Alternatives considered**:
- `iconv` bindings: rejected for cross-platform issues on Windows.
- Pre-conversion to UTF-8 temp file: rejected for disk I/O overhead.

### 5. Delimiter auto-detection strategy

**Decision**: Delimiter is inferred from file extension: `.csv` → comma,
`.tsv`/`.tab` → tab. Manual override via `--delimiter` accepts named values
(`tab`, `comma`, `pipe`, `semicolon`) or any single ASCII character. No
content-based sniffing is performed.

**Rationale**: Extension-based detection is deterministic and fast. Content
sniffing (counting delimiters per line) is fragile with quoted fields and adds
latency. The manual override covers edge cases.

**Alternatives considered**:
- Statistical delimiter sniffing: rejected for fragility with multi-line quoted
  fields and ambiguous cases (semicolons in addresses).

### 6. Error handling architecture

**Decision**: The crate uses a hybrid error strategy: `anyhow::Result<T>` at
command boundaries for rich context chains, and `thiserror`-derived enums for
structured errors within modules. The `?` operator propagates errors upward.
The CLI layer formats errors for human consumption and sets exit codes
(0 = success, non-zero = failure).

**Rationale**: `anyhow` excels at ad-hoc context attachment (`with_context`),
while `thiserror` enables pattern-matching on specific failure modes in tests
and internal logic. The combination is idiomatic Rust for CLI applications.

**Alternatives considered**:
- Pure `thiserror` everywhere: rejected for excessive boilerplate in boundary
  code that just needs to add context strings.
- Pure `anyhow` everywhere: rejected for loss of structured error matching in
  internal modules.

### 7. Streaming sort via index

**Decision**: When a prebuilt index variant matches the requested sort, the
process command reads byte offsets from the index's `BTreeMap` in sorted order
and seeks to each offset in the source CSV file, emitting rows without buffering
the entire dataset. This makes sort cost proportional to I/O (seek + read per
row) rather than O(n log n) in-memory sort.

**Rationale**: For multi-GB files, in-memory sort is impractical. Index-
accelerated sort trades disk seeks for memory, enabling sorts on arbitrarily
large files with O(1) memory overhead (excluding the index structure itself).

**Alternatives considered**:
- External merge sort: rejected for implementation complexity and temp file
  management.
- Memory-mapped file sort: rejected for portability issues and non-deterministic
  paging behavior.

### 8. Schema YAML format compatibility

**Decision**: Schemas are persisted as YAML files using `serde_yaml`. The format
includes `schema_version` (optional), `has_headers` flag, and a `columns` array
where each entry has `name`, `datatype`, optional `name_mapping` (rename),
optional `replace` array (value replacements), and optional `datatype_mappings`
array. The `DecimalSpec` serializes inline with `precision` and `scale` fields.

**Rationale**: YAML is human-readable, widely supported in data engineering
toolchains, and diff-friendly in version control. `serde_yaml` provides
automatic round-trip serialization.

**Alternatives considered**:
- JSON schema: rejected for poor readability and no comment support.
- TOML: rejected for awkward nested array representation.

## Resolved Clarifications

All technical context fields were resolved from codebase analysis. No NEEDS
CLARIFICATION markers remain. The following items were pre-resolved:

| Item | Resolution |
|------|------------|
| Rust edition | 2024 (confirmed from `Cargo.toml`) |
| Join subcommand status | Dormant — commented out in CLI dispatch, excluded from baseline spec |
| Performance targets | Streaming by default; index sort for large-file acceleration |
| Memory constraints | Bounded by streaming design; only in-memory sort fallback is unbounded |
| Platform matrix | Windows, Linux (glibc + musl), macOS (aarch64 + x86_64) |
