# Statistics & Frequency Deep Dive

Comprehensive guide to the `stats` command: numeric & temporal summaries, frequency counting, filtering interplay, performance considerations, and edge cases.

## Overview

`csv-managed stats` produces summary metrics (count, min, max, mean, median, standard deviation) for numeric & temporal columns, or distinct value counts using `--frequency`.

## Supported Datatypes

| Category | Types | Notes |
|----------|-------|-------|
| Numeric | Integer, Float, decimal(p,s), Currency | Decimal & Currency normalized to numeric values for aggregation (scale preserved for output formatting) |
| Temporal | Date, DateTime, Time | Converted to numeric offsets (days from CE; epoch seconds; seconds from midnight) for calculations |

String, Guid, Boolean are excluded from summary metrics (unless frequency mode is used).

## Temporal Conversion Model

| Type | Internal Aggregation Unit | Example Conversion |
|------|---------------------------|--------------------|
| Date | Days from Common Era | 2024-01-06 → integer day index |
| DateTime | Seconds from Unix Epoch (UTC naive) | 2024-01-06 05:57:30 → epoch seconds |
| Time | Seconds from Midnight | 08:00:00 → 28800 |

Values are converted, aggregated, then rendered back to canonical formats for min/max/median/mean. Std dev uses `days` for Date and `seconds` for DateTime/Time.

## Metrics Definitions

| Metric | Definition | Notes |
|--------|------------|-------|
| count | Number of non-empty, successfully parsed values | Empty/placeholder values omitted |
| min / max | Extreme values under total ordering | Currency & Decimal respect numeric ordering after parsing |
| mean | Arithmetic average | Computed on converted numeric form; re-render for temporal types |
| median | Middle value (or average of two middle values) | For large columns median requires buffering parsed values |
| std_dev | Population standard deviation (σ) | Unit: numeric; for temporal re-labeled with suffix (days / seconds) |

## Frequency Mode

Enable with `--frequency` to list distinct values per column. Combine with:

- `--top N` (0 = all) to limit reported distincts.
- `-C/--columns` to focus on specific columns.

### Output Columns (Frequency)

| Column | Meaning |
|--------|---------|
| column | Column name |
| value | Distinct value (rendered canonical) |
| count | Occurrences |
| percent | (count / total rows scanned) * 100 |

## Filtering Interplay

Filters (`--filter` and/or `--filter-expr`) apply BEFORE aggregation or distinct counting:

1. Input row parsed & mapped (if schema supplied).
2. Filter predicates evaluated.
3. Only rows passing all predicates contribute to stats/frequency.

Recommended: verify filter correctness using a preview (`process --preview`) or a limited run, then pipe into `stats` if header shape unchanged.

## Median Performance Notes

- Median requires storing all parsed values for each selected column.
- For extremely large datasets consider projecting fewer numeric columns or omitting median (future flag may control). Currently median computed automatically.
- Decimals & Currency: parsed into precise numeric forms; buffer uses stable representation.

## Decimal & Currency Handling

| Aspect | Behavior |
|--------|----------|
| Precision & Scale | Enforced during parsing via schema; invalid rows excluded from aggregation |
| Rounding | Already applied if mapping strategy used (round/truncate) prior to stats stage |
| Output | Mean/median rendered at declared scale; std_dev uses raw numeric scale before formatting |

## Examples

### Basic Numeric Stats

```powershell
csv-managed stats -i data/orders.csv -m data/orders-schema.yml -C amount -C tax
```

### Temporal Stats Subset

```powershell
csv-managed stats -i data/orders_temporal.csv -m data/orders_temporal-schema.yml \
  --columns ordered_at --columns ordered_at_ts --columns ship_time
```

### Frequency Counts (Top 10)

```powershell
csv-managed stats -i data/orders.csv -m data/orders-schema.yml --frequency --top 10 -C status
```

### Filter + Stats

```powershell
csv-managed stats -i data/orders.csv -m data/orders-schema.yml \
  --filter "status = shipped" --filter "amount >= 100" -C amount -C tax
```

### Pipeline (Process → Stats)

```powershell
Get-Content .\tests\data\big_5_players_stats_2023_2024.csv | \
  .\target\release\csv-managed.exe process -i - --schema .\tests\data\big_5_players_stats-schema.yml --filter "Performance_Gls >= 5" | \
  .\target\release\csv-managed.exe stats -i - --schema .\tests\data\big_5_players_stats-schema.yml -C Performance_Gls
```

## Edge Cases & Troubleshooting

| Symptom | Cause | Resolution |
|---------|-------|-----------|
| Zero rows in output | Filters exclude all rows | Relax or validate filters with a preview |
| No numeric columns reported | Projection removed numeric fields | Include them via `-C` or adjust upstream projection |
| Temporal column missing | Schema absent or type inferred as String | Supply correct schema or re-infer |
| Currency/Decimal invalid counts | Parsing failures (scale/precision) | Verify schema spec; clean source or mapping round/truncate |
| Long runtime / high memory | Too many wide columns with large cardinality median | Limit columns or pre-filter rows |

## Performance Tips

1. Filter early to reduce row set.
2. Restrict columns via `-C` when exploring.
3. Avoid unnecessary median computation on massive low-signal columns (project out non-essential numeric fields).
4. Use indexing in upstream `process` sort stage rather than sorting inside stats (stats does not sort rows; upstream sort is optional but can aid reproducible ordering when frequency scanning large sets).
5. Use `--limit` for exploratory runs; remove when ready for full aggregation.

## Recommended Workflow

1. Infer & verify schema.
2. Quick preview using `process --preview` or small `--limit` stats run.
3. Add filters & column selection.
4. Run full stats; capture output for monitoring or regression tracking.

## Data Quality Integration

Pair `schema verify` before stats to ensure aggregated metrics are based on valid typed rows. Consider failing CI if invalid rows exceed threshold before computing metrics.

## Future Enhancements

- Optional suppression of median for ultra-large profiles.
- Percentile calculations (P90/P95) for numeric & temporal columns.
- Streaming approximate distinct counts (HyperLogLog) for very high cardinality frequency mode.
- Grouped aggregations (multi-column grouping) for lightweight summarization.

## Quick Reference Table

| Flag | Purpose |
|------|---------|
| `-C/--columns` | Restrict stats/frequency to listed columns |
| `--frequency` | Switch from summary metrics to distinct counts |
| `--top N` | Limit distinct values (frequency mode) |
| `--limit N` | Limit number of rows scanned (0 = all) |
| `--filter` / `--filter-expr` | Row selection prior to aggregation |
| `--schema` | Provide datatype context (highly recommended) |

## Validation Snippet (Rust)

```rust
Command::cargo_bin("csv-managed")?
  .args(["stats","-i","tests/data/orders.csv","-m","tests/data/orders-schema.yml","--columns","amount"]) 
  .assert()
  .success();
```

## Summary

The `stats` command delivers performant streaming aggregation by converting temporal datatypes to numeric domains and enforcing schema-driven numeric precision. Combine with upstream filtering and column projection for scalable analytics over very large CSV datasets.
