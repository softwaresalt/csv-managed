# Designing Multi-Stage Pipelines

This guide expands on README Streaming & Pipelines with deeper patterns for chaining commands (`process`, `stats`, `append`, future `join`) while preserving performance and schema correctness.

## Goals

* Stream large datasets without intermediate files.
* Keep typed operations valid across stages.
* Normalize encodings early.
* Prevent header/schema drift.

## Stage Categories

| Category | Examples | Header Change | Safe With Original Schema? |
|----------|----------|---------------|-----------------------------|
| Row-only filters/sort | `--filter`, `--filter-expr`, `--sort`, `--limit` | None | Yes |
| Projection/exclusion | `--columns`, `--exclude-columns` | Removes columns | Only w/ updated schema or string-only downstream |
| Derivation | `--derive` | Adds columns | No (needs new schema) |
| Encoding | `--input-encoding`, `--output-encoding` | None | Yes |
| Boolean format | `--boolean-format` | None | Yes |
| Delimiter change | `--output-delimiter` | None | Yes |
| Datatype mappings | `--apply-mappings` | None | Yes |
| Append | `append` | Must match baseline | Yes if identical |

## Header Shape Invariance Between Typed Stages

`stats` and `schema verify` require an exact match (columns + order). When piping `process | stats` keep header unchanged. Avoid derives and projection unless you evolve the schema first.

### Why Exact Matching?

Parsers are bound by position; mismatch yields parse errors or silent type misalignment.

## Changing Column Shape Safely

1. Derive then materialize: `process --derive ... -o out.csv`; infer new schema; run typed downstream stages.
2. Use `process --emit-schema <path>` (optionally `--emit-evolution-base existing-schema.yml`) to auto-export the transformed layout and emit a schema-evolution diff without leaving the streaming pipeline.
3. String-only downstream (omit `--schema`) if later steps just textual.

## Schema Evolution Examples

### Streaming Emit + Evolution Report

```powershell
Get-Content .\tests\data\stats_schema.csv | \
  .\target\release\csv-managed.exe process -i - `
    --schema .\tests\data\stats_schema-schema.yml `
    --derive double_price:Float=price*2 `
    --emit-schema .\tmp\stats_with_extra-schema.yml `
    --emit-evolution-base .\tests\data\stats_schema-schema.yml | \
  .\target\release\csv-managed.exe stats -i - `
    --schema .\tmp\stats_with_extra-schema.yml `
    -C double_price
```

The process stage writes the transformed schema to `stats_with_extra-schema.yml` and emits an evolution report (`stats_with_extra-schema.evo.yml`) that lists newly added or changed columns. The emitted schema can be fed directly into downstream typed commands (stats, verify, append) without re-running `schema infer`.

### PowerShell: Derive, Infer, Reuse

```powershell
./target/release/csv-managed.exe process ` 
  -i ./tests/data/stats_schema.csv ` 
  --schema ./tests/data/stats_schema-schema.yml ` 
  --derive extra=price*2 ` 
  -o ./tmp/stats_with_extra.csv

./target/release/csv-managed.exe schema infer ` 
  -i ./tmp/stats_with_extra.csv ` 
  -o ./tmp/stats_with_extra-schema.yml ` 
  --sample-rows 0

./target/release/csv-managed.exe stats ` 
  -i ./tmp/stats_with_extra.csv ` 
  -m ./tmp/stats_with_extra-schema.yml ` 
  -C price -C extra
```

### Bash / zsh Variant

```bash
./target/release/csv-managed process \ 
  -i tests/data/stats_schema.csv \ 
  --schema tests/data/stats_schema-schema.yml \ 
  --derive extra=price*2 \ 
  -o tmp/stats_with_extra.csv

./target/release/csv-managed schema infer \ 
  -i tmp/stats_with_extra.csv \ 
  -o tmp/stats_with_extra-schema.yml \ 
  --sample-rows 0

./target/release/csv-managed stats \ 
  -i tmp/stats_with_extra.csv \ 
  -m tmp/stats_with_extra-schema.yml \ 
  -C price -C extra
```

### cmd.exe Variant

```batch
target\release\csv-managed.exe process ^ 
  -i tests\data\stats_schema.csv ^ 
  --schema tests\data\stats_schema-schema.yml ^ 
  --derive extra=price*2 ^ 
  -o tmp\stats_with_extra.csv

target\release\csv-managed.exe schema infer ^ 
  -i tmp\stats_with_extra.csv ^ 
  -o tmp\stats_with_extra-schema.yml ^ 
  --sample-rows 0

target\release\csv-managed.exe stats ^ 
  -i tmp\stats_with_extra.csv ^ 
  -m tmp\stats_with_extra-schema.yml ^ 
  -C price -C extra
```

These flows illustrate the current manual schema evolution path: derive and materialize data, regenerate a schema describing the new header layout, then continue chaining typed stages with the updated schema file.

## Encoding First Pattern

```powershell
Get-Content .\tmp\big_5_windows1252.csv | \
  .\target\release\csv-managed.exe process -i - --input-encoding windows-1252 --schema .\tests\data\big_5_players_stats-schema.yml | \
  .\target\release\csv-managed.exe stats -i - --schema .\tests\data\big_5_players_stats-schema.yml -C Performance_Gls
```

## Recommended Patterns

| Pattern | Purpose | Example (Concept) |
|---------|---------|------------------|
| Filter → Stats | Reduce rows before aggregate | Filter rows, then compute stats |
| Encode → Filter → Stats | Decode then aggregate | Normalize encoding, filter, then stats |
| Derive → Materialize → New Schema → Stats | Safely add columns | Derive & write, infer new schema, stats |
| Verify → Process → Append | Enforce data quality then union | Verify datasets, transform, append |

## Anti-Patterns

| Anti-Pattern | Issue | Fix |
|--------------|------|-----|
| Derive then reuse old schema in stats | Extra column mismatch | New schema |
| Column subset then stats with full schema | Missing columns | Keep full shape or evolve schema |
| Mixed encodings mid-pipeline | Parse errors | Normalize first |

## Performance Tips

* Filter early.
* Keep derived expressions lean.
* Use indexes for large sorts.
* Limit numeric columns when computing medians.

### Additional Core Principles (Quick Reference)

1. Stream once – avoid materializing unless header shape changes.
2. Normalize encodings to UTF-8 early.
3. Re‐infer schema immediately after structural changes (add/drop/reorder columns).
4. Keep upstream stages orthogonal: filtering, then projection, then derivation (if needed), then stats.
5. Prefer a single `process` stage rather than multiple small ones to reduce parse overhead.

## Additional Pipeline Patterns

### Multi-File Union With Pre-Filter

```bash
(cat part1.csv; cat part2.csv) \
  | csv-managed append -i - -i extra/part3.csv --schema schemas/layout.yml --dedupe --output tmp/union.csv
```

### Snapshot Validation (Drift Detection)

```bash
csv-managed schema probe -i data/big.csv --sample-rows 250 --snapshot snapshots/big.snap
# Later in CI
cat data/big.csv | csv-managed schema infer -i - --sample-rows 250 --snapshot snapshots/big.snap
```

### Mixed Input + Streamed Append

```bash
cat data/new_rows.csv \
  | csv-managed append -i - -i data/existing.csv --schema schemas/layout.yml --output tmp/combined.csv
```

## Header & Schema Evolution Guidance

| Change Type                 | Needs New Schema? | Notes |
|----------------------------|-------------------|-------|
| Pure rename (name_mapping) | No                | Original or mapped name accepted downstream. |
| Added derived column       | Yes               | Infer or author updated schema before typed stage. |
| Dropped column             | Yes               | Old schema will report mismatch. |
| Reordered columns          | Yes (currently)   | Order significant for typed parsing. |
| Encoding normalization     | No                | Provided header tokens unchanged. |

## Troubleshooting Quick Ref

| Symptom                                   | Likely Cause                               | Action |
|-------------------------------------------|---------------------------------------------|--------|
| Header mismatch after `process` derive    | Schema not regenerated                      | Infer new schema over transformed output. |
| Stats shows zero numeric columns          | Projection dropped numeric fields           | Preserve numeric columns or add `-C` flags. |
| Currency/decimal parse failures           | Wrong input encoding or untrimmed tokens    | Normalize encoding; inspect raw values. |
| Pipeline halts mid-chain                  | Upstream error surfaced late                | Run each stage standalone; enable `RUST_LOG=info`. |
| Unexpected string typing after inference  | Insufficient sampling / mixed tokens        | Increase `--sample-rows` or add explicit override. |

## Environment & CI Example

```bash
set -euo pipefail
csv-managed schema probe -i data/big.csv --sample-rows 250 --snapshot snapshots/big.snap
cat data/big.csv | csv-managed process -i - --schema schemas/big.yml --filter "score>=50" --columns id --columns score | \
  csv-managed stats -i - --schema schemas/big.yml -C score
```

Include `snapshots/*.snap` in version control to detect structural drift intentionally.

## Roadmap

* Streaming join stage with schema suggestion.
* Primary key + hash signature integration.

## Cross-Shell Example (Filter → Stats)

PowerShell:

```powershell
Get-Content .\tests\data\big_5_players_stats_2023_2024.csv | \
  .\target\release\csv-managed.exe process -i - --schema .\tests\data\big_5_players_stats-schema.yml --filter "Performance_Gls >= 5" --columns Player --columns Performance_Gls | \
  .\target\release\csv-managed.exe stats -i - --schema .\tests\data\big_5_players_stats-schema.yml -C Performance_Gls
```

cmd.exe:

```batch
type tests\data\big_5_players_stats_2023_2024.csv | target\release\csv-managed.exe process -i - --schema tests\data\big_5_players_stats-schema.yml --filter "Performance_Gls >= 5" --columns Player --columns Performance_Gls | target\release\csv-managed.exe stats -i - --schema tests\data\big_5_players_stats-schema.yml -C Performance_Gls
```

Bash / zsh:

```bash
cat tests/data/big_5_players_stats_2023_2024.csv | ./target/release/csv-managed process -i - --schema tests/data/big_5_players_stats-schema.yml --filter "Performance_Gls >= 5" --columns Player --columns Performance_Gls | ./target/release/csv-managed stats -i - --schema tests/data/big_5_players_stats-schema.yml -C Performance_Gls
```

## Checklist

1. Header unchanged? If not, new schema.
2. Encoding normalized.
3. Filters early.
4. Derives consolidated.
5. Materialize before schema-dependent downstream ops if shape changed.

## Summary

Preserve header shape between typed streaming stages; if you must change it, generate an updated schema. Normalize encoding first, filter early, and materialize after structural transformations.
