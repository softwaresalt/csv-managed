# Indexing & Sorting Guide

This guide explains how the `index` and `process --sort` features work, how they interact, and how to choose the right strategy for high‑performance sorting across large CSV datasets.

---

## 1. Overview

The application supports two sorting paths:

1. Pure in‑memory sorting (no index) – rows are read, typed, optionally filtered/mapped, then sorted entirely in memory.
2. Index‑accelerated sorting – a prebuilt B‑Tree style index orders row byte offsets for one or more leading sort columns and directions. Any remaining (non‑covered) sort columns are sorted in memory only within prefix buckets.

An index can contain multiple *variants*. Each variant represents a distinct ordered key: a sequence of columns with individual ascending/descending directions. Variants are useful when you frequently sort by different leading prefixes (e.g., `date` or `date,status` or `date,status,region`).

---

## 2. Index File Formats

Current index on-disk version: **2** (`INDEX_VERSION = 2`).

- Version 2 supports multiple variants and mixed sort directions per column.
- A fallback loader converts legacy single‑variant (version 1) files automatically. When legacy decoding triggers you may see the context message: *"Reading legacy index file format"*. This indicates upgrade-in-place succeeded, not an error condition by itself.

---

## 3. Building Indexes

### 3.1 Basic Ascending Index

```powershell
csv-managed.exe index -i data/orders_temporal.csv -o tmp/ordered_at.idx -m data/orders_temporal-schema.yml -C ordered_at
```

The `-C/--columns` flag produces a single ascending variant (deprecated once `--spec` or `--combo` are used).

### 3.2 Explicit Variant Specifications (`--spec`)

You can repeat `--spec` to define multiple variants in one file:

```powershell
csv-managed.exe index -i data/orders_temporal.csv -o tmp/orders_variants.idx -m data/orders_temporal-schema.yml `
  --spec "recent=ordered_at:desc" `
  --spec "ordered_at:asc,ship_time:desc" `
  --spec "ordered_at:asc,status:asc"
```

Rules:

- Syntax: `[name=]col[:asc|desc][,col[:asc|desc]]...`
- Name optional; if omitted variant has no identifier (still matchable by best-fit logic).
- At least one column required.

### 3.3 Combination Expansion (`--combo`)

Generates *prefix* variants across Cartesian products of directions:

```powershell
csv-managed.exe index -i data/orders_temporal.csv -o tmp/orders_combo.idx -m data/orders_temporal-schema.yml `
  --combo "orders=ordered_at:asc|desc,status:asc|desc,ship_time:asc"
```

This produces variants for:

- `ordered_at` (asc & desc)
- `ordered_at,status` (asc/asc, asc/desc, desc/asc, desc/desc)
- `ordered_at,status,ship_time` (each status direction × ordered_at direction; ship_time asc fixed)

Names are auto-generated from prefix + directions (e.g., `orders_ordered_at-asc_status-desc_ship_time-asc`).

### 3.4 Limiting Rows for Prototyping

```powershell
csv-managed.exe index -i data/big_5_players_stats_2023_2024.csv -o tmp/big5_perf.idx -m data/big_5_players_stats-schema.yml --spec "Performance_Gls:desc,Performance_Ast:desc" --limit 5000
```

Use `--limit` to speed up exploratory index builds on huge files.

### 3.5 Decimal & Currency Columns

Provided a schema declares types:

```powershell
csv-managed.exe index -i data/decimal_measurements.csv -o tmp/decimal_measurements.idx -m data/decimal_measurements-schema.yml --spec "measurement_exact:asc"
```

Precision & scale enforced during index build; invalid values abort the build.

---

## 4. Using Indexes in `process`

### 4.1 Basic Accelerated Sort

```powershell
csv-managed.exe process -i data/orders_temporal.csv -m data/orders_temporal-schema.yml `
  --index tmp/orders_variants.idx --sort ordered_at:desc --columns ordered_at,status --limit 50 -o tmp/recent_orders.csv
```

If an index variant matches the leading requested sort columns & directions, byte offsets stream in that order. Remaining non‑covered sort columns (if any) are sorted in‑memory *per prefix bucket*.

### 4.2 Selecting a Named Variant

```powershell
csv-managed.exe process -i data/orders_temporal.csv -m data/orders_temporal-schema.yml `
  --index tmp/orders_variants.idx --index-variant recent --sort ordered_at:desc --columns ordered_at,status --limit 25 --preview
```

Requirements:

- At least one `--sort` directive provided.
- Selected variant must match the full prefix portion of the requested sort signature.

### 4.3 Mixed Direction with Remainder Columns

```powershell
csv-managed.exe process -i data/orders_temporal.csv -m data/orders_temporal-schema.yml `
  --index tmp/orders_combo.idx --sort ordered_at:asc --sort status:desc --sort ship_time:asc -o tmp/sorted.csv
```

If the variant covers only `ordered_at,status`, then rows sharing the same `(ordered_at,status)` pair are locally sorted by `ship_time` in memory before emission.

### 4.4 Fallback Behavior

If no variant matches the requested sort signature:

- App logs a debug message that index was not used.
- Full in-memory sort executes (all rows parsed before sort). For very large files prefer building an index variant matching your primary sort path.

### 4.5 Multi‑Datatype Sorting

All declared datatypes implement a total ordering within their variant:

- Integer, Float: numeric comparison (Floats via `total_cmp`).
- Decimal & Currency: scale‑aware `Decimal` comparisons, enforcing precision/scale during parsing.
- Date, Time, DateTime: chronological ordering.
- Boolean: `false < true`.
- Guid: lexicographic (canonical UUID string forms produce deterministic ordering).
- String: ordinal (UTF‑8 byte order); consider normalizing case via mappings if you need case-insensitive sorting.
- Empty cells (parsed as `None`) sort before any concrete value.

---

## 5. Variant Matching Logic

Given requested sort directives `[(col1,dir1),(col2,dir2),...]`:

1. Named variant selection (`--index-variant`) validates exact match of columns/directions for the variant length.
2. Otherwise, best match chooses the variant with the **longest** column prefix that aligns exactly with the start of the requested sort sequence.
3. If multiple candidates have the same covered length, the first encountered wins (variants creation order). Prefer giving distinct names to critical variants for determinism.

---

## 6. Performance Considerations

| Scenario | Recommended | Notes |
|----------|-------------|-------|
| Large file, repeatable single sort | Single variant index | Minimal storage overhead |
| Several common prefixes | Use `--combo` | Avoid building unnecessary deep combinations |
| Many unrelated sort orders | Separate smaller indexes | Avoid one huge file with dozens of variants |
| Mixed directions frequently | Explicit `--spec` variants | Reduces combinatorial explosion from `--combo` |
| Changing schema/datatype mappings | Rebuild index | Mappings alter typed values → key ordering may shift |

Memory impact: index processing keeps only the active *bucket* of rows in memory when the variant covers a prefix shorter than the full sort plan. Full in‑memory sort holds all rows.

---

## 7. Troubleshooting

| Symptom | Cause | Resolution |
|---------|-------|-----------|
| "Column 'X' not found" | Spec/Combo references missing header | Correct column name or regenerate schema for reference |
| Index ignored | No variant matches requested sort prefix | Build a matching variant or adjust sort directives order |
| Legacy format message | Old index loaded, converted | Safe to ignore; consider rebuilding to v2 for variants |
| Slow processing with sort | In‑memory fallback | Add index variant covering leading sort columns |
| Sorting incorrect for decimals | Precision/scale mismatch or invalid data | Verify schema spec (e.g., `decimal(28,6)`), re‑validate input |

---

## 8. Best Practices Summary

- Put most selective column first in variants for efficient prefix bucketing.
- Use `--combo` sparingly—prefer explicit `--spec` for curated common paths.
- Keep variant names short but descriptive (e.g., `recent`, `asc_perf`, `geo_country_desc`).
- Rebuild indexes after datatype mapping changes that materially transform key columns.
- Store indexes alongside source data with a naming convention: `<base>.<pattern>.idx`.

---

## 9. Example End-to-End Flow

```powershell
# Infer schema
csv-managed.exe schema infer -i data/orders_temporal.csv -o tmp/orders_temporal-schema.yml --sample-rows 0

# Build index variants
csv-managed.exe index -i data/orders_temporal.csv -o tmp/orders.idx -m tmp/orders_temporal-schema.yml `
  --spec "recent=ordered_at:desc" `
  --spec "ordered_at:asc,status:asc" `
  --combo "ordered_at:asc|desc,status:asc"

# Accelerated processing using named variant
csv-managed.exe process -i data/orders_temporal.csv -m tmp/orders_temporal-schema.yml `
  --index tmp/orders.idx --index-variant recent --sort ordered_at:desc --columns ordered_at,status --limit 100 -o tmp/recent_orders.csv
```

---

## 10. Future Enhancements (Roadmap Hooks)

- Directory-wide multi-file indexing (planned).
- Primary key & hash signatures for fast duplicate detection.
- Partial materialization heuristics for composite key subsets.

---

*Last updated: 2025-10-26.*
