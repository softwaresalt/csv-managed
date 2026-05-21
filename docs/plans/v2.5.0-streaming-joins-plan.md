# v2.5.0 Feature Planning: Streaming & Pipeline-Friendly Joins/Lookups

## Overview

Rather than merging `join` into `process` (which would inflate flag surface and complexity), v2.5.0 will focus on strengthening pipeline composability so users can chain:

```text
csv-managed process ... | csv-managed join --left - --right file.csv ... | csv-managed process ...
```

This approach aligns with the Unix philosophy (small, sharp tools) while preserving clarity: `process` transforms one stream; `join` combines two. We enhance `join` to accept a streamed left side (`stdin`) without requiring an explicit schema file and interoperate cleanly with prior transformations.

## Goals

1. Enable streaming left input (`--left -`) with automatic header-based schema inference.
2. Allow relaxed type matching when left schema is inferred (String) and right schema provides stronger types for join keys.
3. Provide `--left-assume-types` override for ambiguous or performance‑critical joins.
4. Maintain current hash join strategy (right fully loaded) while improving error messages & UX.
5. Document pipeline patterns & recommend multi-step transformations over a monolithic merged command.
6. Add integration tests for end-to-end pipelines (process → join → process).
7. Preserve existing `join` command (no deprecation yet); revisit merge only if multi-source orchestration demands it.

## Non-Goals (v1.6.0)

- Index-assisted multi-source join optimization.
- Streaming right-side input from stdin (still requires a file path).
- Merge join (ordered, low-memory) implementation.
- Multi-way joins (chaining >2 sources in one invocation).

## Proposed Enhancements & Implementation Details

### 1. Header-Based Inference for Left Stream

If `--left -` and `--left-schema` absent:

- Read first line (headers) from stdin.
- Construct in-memory `Schema` with all columns typed as `String` and mark `inferred = true` (internal flag).
- Continue row streaming after headers.

```rust
fn infer_stream_schema<R: std::io::Read>(reader: &mut csv::Reader<R>, enc: &'static Encoding) -> Result<Schema> {
    let headers = io_utils::reader_headers(reader, enc)?;
    Ok(Schema::from_headers(&headers).with_inferred_flag())
}
```

### 2. Relaxed Key Type Validation

Modify join key validation:

- If left type == `String` and `schema.is_inferred()`, accept right type as authoritative.
- Parse left key values according to right datatype during key extraction; surface parse errors with row/column context.

### 3. `--left-assume-types` Flag

Allows specifying stronger types for streamed left columns without a full schema file:

```text
--left-assume-types id:integer,created_at:datetime,status:string
```

Parsing:

```rust
fn parse_assumed_types(specs: &[String], schema: &mut Schema) -> Result<()> {
    for spec in specs { /* split name:type, update schema columns */ }
    Ok(())
}
```

### 4. Error Message Improvements

- On type mismatch: suggest `--left-assume-types` or supplying `--left-schema`.
- On parse failure for inferred left key: report original token, expected type, row number.

### 5. Pipeline Documentation & Examples

Add a README section “Streaming Joins via Pipelines” with several real-world examples (see Code Examples below).

### 6. Test Additions

| Test | Purpose |
|------|---------|
| `join_stream_left_infer.rs` | Inner join with inferred left header schema. |
| `join_stream_left_assume_types.rs` | Overrides to enforce integer/datetime parsing. |
| `pipeline_process_join_process.rs` | Compose transform → join → derive + filter. |
| `join_stream_left_type_error.rs` | Helpful error on mismatch. |
| `join_stream_left_full_outer.rs` | Full outer join with streamed left input. |

### 7. Performance Considerations

- Right side is fully hashed; warn if row count > threshold (future: `--join-warn-threshold`).
- Left stream processed row-by-row; memory = right hash + output buffering.
- Sorting after join remains a separate `process` invocation.

### 8. Future Extensibility Hooks

- Merge join option (`--strategy merge`) when both sides sorted by key.
- Multi-source chaining via batch JSON definition.
- Optional `--right-stdin` with spool temp file & size guard.
- Index-aware join (reuse left index for deterministic key ordering).

## Acceptance Criteria

1. `csv-managed join --left - --right right.csv --left-key id --right-key id` succeeds when piped input provides matching headers.
2. `process | join | process` chains work without temp files.
3. `--left-assume-types` enforces parsing for specified columns.
4. Descriptive errors for invalid left key tokens (non-integer for integer key) show row & expected type.
5. New tests pass; existing join tests unaffected.
6. README documents pipeline usage with ≥3 examples.

## Risks & Mitigations

| Risk | Mitigation |
|------|------------|
| Silent type mismatch due to inferred String | Parse left using right types; fail fast with context. |
| Startup latency reading headers from stdin | Single header read is negligible; document behavior. |
| Confusion between `--left-schema` and `--left-assume-types` | Add README FAQ section. |
| Memory blowup on huge right side | Document trade-off; future threshold warning. |

## Code Examples

### Basic Inner Join After Transform

```powershell
csv-managed process -i orders.csv --filter "status=shipped" --columns order_id,customer_id,total |
  csv-managed join --left - --right customers.csv --left-key customer_id --right-key id --type inner |
  csv-managed process -i - --derive 'high_value=total>500' --filter-expr 'high_value' --limit 25
```

### Left Join With Preview

```powershell
csv-managed process -i orders.csv --columns order_id,customer_id,total |
  csv-managed join --left - --right customers.csv --left-key customer_id --right-key id --type left |
  csv-managed process -i - --columns order_id,total,name --row-numbers --preview
```

### Assumed Types for Streamed Left

```powershell
producer.exe |
  csv-managed join --left - --right dim_products.csv --left-key product_id --right-key id \
    --left-assume-types product_id:integer
```

### Full Outer Join Pipeline

```powershell
csv-managed process -i metrics_a.csv --derive 'day=date_format(timestamp,"%Y-%m-%d")' |
  csv-managed join --left - --right metrics_b.csv --left-key day --right-key day --type full |
  csv-managed process -i - --filter 'status = active' --limit 100
```

### Post-Join Derived Columns

```powershell
csv-managed process -i sales.csv --columns order_id,customer_id,amount |
  csv-managed join --left - --right customers.csv --left-key customer_id --right-key id --type inner |
  csv-managed process -i - --derive 'amount_with_tax=amount*1.0825' --sort amount_with_tax:desc --limit 10
```

## FAQ (Draft)

**Q: When do I need `--left-schema`?** When you require full type validation on all left columns, not just join keys.  
**Q: Why not merge `join` into `process`?** Avoids flag bloat; pipelines keep mental model clean.  
**Q: Can I sort joined output directly?** Yes—pipe into a second `process` invocation and use `--sort`.  
**Q: How are replacements applied on left stream?** Any prior `process` stage applied them; `join` consumes normalized values.  

## Implementation Checklist

- [ ] Add inferred schema path for `--left -` without `--left-schema`.
- [ ] Add `Schema::is_inferred()` flag.
- [ ] Implement `--left-assume-types` parsing.
- [ ] Adjust join key type validation & left key parsing.
- [ ] Improve error messages (row, column, expected type).
- [ ] Add new pipeline tests.
- [ ] Update README & cli-help snapshots.
- [ ] Document memory characteristics & future roadmap items.

## Release Notes (Draft)

"v1.6.0 introduces streaming-friendly joins: pipe transformed data directly into `join` via stdin, infer left-side schema automatically, override key types with `--left-assume-types`, and compose multi-step workflows without temporary files."
