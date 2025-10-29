# Operational Notes: Performance, Errors, Logging, Testing

## Performance Principles

| Principle | Rationale | Action |
|-----------|-----------|--------|
| Filter early | Shrinks row set before heavy operations | Use `--filter`/`--filter-expr` prior to sort/derive |
| Use indexes for large sorts | Avoid full in-memory sorts | Build variants with `index --spec` |
| Narrow projections | Reduce per-row evaluation cost | Apply `-C` early |
| Manage decimals/currency only where needed | Parsing overhead | Restrict high-precision types to critical columns |
| Avoid wide median computations | Median buffers column values | Compute only necessary stats columns |

## In-Memory vs Indexed Sorting

Fallback occurs when no variant covers the requested sort prefix. For repeated sorts build an index variant to minimize buffering.

## Error Handling Strategy

- Rich context: I/O, parsing, expression, schema, index errors use `anyhow` contexts.
- Fast fail: unknown column names, invalid mapping strategy combinations.
- Invalid UTF-8 rows reported; not silently skipped.
- Decimal precision overflow triggers explicit invalid cell reporting in verification.

## Logging

Set environment variable:

```powershell
$env:RUST_LOG='csv_managed=debug'
```

Debug surfaces: inference voting hints, index selection, mapping application. Use selectively to avoid throughput impact.

## Testing Approaches

- Unit tests: datatype parsing, mapping strategies, header detection algorithm boundaries.
- Integration tests: pipeline chaining (`stdin_pipeline`), schema verification, indexing & sorting accuracy.
- Snapshot tests: inference output locked with `--snapshot` (commit snapshot file).

### Example (Rust `assert_cmd` Snippet)

```rust
Command::cargo_bin("csv-managed")?
  .args(["process","-i","-","--schema", schema_path, "--columns","Player","--limit","3"]) 
  .write_stdin(std::fs::read_to_string(input_path)?)
  .assert()
  .success();
```

## Troubleshooting Quick Table

| Symptom | Cause | Resolution |
|---------|-------|-----------|
| Slow sort | In-memory fallback | Build index variant for prefix |
| Stats show zero rows | Filter removed all | Loosen conditions / test with `--limit` |
| Boolean inferred as String | Unclassified tokens | Add replacements or override |
| Currency parsing fails | Mixed scale (e.g. 3 decimals) | Clean source or map/override |
| Header mismatch downstream | Derived / projection changed shape | Infer new schema over transformed file |

## CI Suggestions

1. Run `schema infer --diff` against committed schema to detect drift.
2. Use snapshots for heuristic/layout regression.
3. Verify critical extracts before append/union operations.

## Roadmap Hooks

- Primary key hashing & duplicate detection.
- Streaming join redesign.
- Automatic schema emission after derived transformations.
