# Header Detection & Headerless CSVs

This page deep dives the header presence heuristic and operational guidance for headerless files.

## Summary

`csv-managed` samples up to the first 6 physical rows to decide whether the first row is a header or data. When determined headerless, synthetic names `field_0..field_{N-1}` are generated and persisted with `has_headers: false` in the schema.

## Algorithm

1. Read up to 6 rows (`HEADER_DETECTION_SAMPLE_ROWS`).
2. Classify each token in row 1:

    - header-like: alphabetic, or matches curated dictionary (`id`, `date`, `amount`, `status`, etc.), and not data-like.
    - data-like: successfully parses as boolean, integer, decimal, float, currency, date, datetime, time, guid.
3. Accumulate header vs data signals comparing row 1 token to same column tokens in subsequent sampled rows.
4. Early headerless if majority of row 1 tokens are data-like or row 1 is empty/whitespace.
5. Tie resolution: (a) compare signal counts; (b) dictionary hits; (c) header-like vs data-like token counts.
6. If headerless, synthesize `field_#` names.

## Override Mechanisms

- CLI: `--assume-header true|false` on `schema probe`/`schema infer` bypasses heuristic.
- Manual: Edit saved schema and set `has_headers: true|false` then rename columns as desired.

## Synthetic Names

Persisted exactly as written; may be referenced directly or via positional aliases (`c0`, `c1`, ...). Rename later using mapping templates or manual YAML edits.

```yaml
schema_version: 1.0
has_headers: false
columns:
  - name: field_0
    datatype: Integer
  - name: field_1
    datatype: Float
```

## FAQ

| Issue | Cause | Resolution |
|-------|-------|------------|
| File treated headerless but has headers | Tokens parsed as numeric/date causing data-like dominance | Force `--assume-header true` or edit schema flag |
| First data row mistaken for header | Alphabetic tokens or dictionary matches; limited sample | Force `--assume-header false`; rename synthetic fields |
| Mixed headered/headerless batch | Source inconsistency | Normalize upstream OR process with two schemas |
| Need guaranteed behavior | Ambiguous format | Always pass `--assume-header` |

## Best Practices

1. Keep clear alphabetical headers (avoid purely numeric names) for reliable auto-detection.
2. When ingesting sensor/data logger output, explicitly force headerless and rename.
3. Commit the schema after corrections; downstream commands honor `has_headers`.
4. Use mapping (`--mapping`) to generate snake_case suggestions even for synthetic names.

## Performance Note

Sampling ≤6 rows is negligible cost; detection does not require full scan and is amortized across inference.

## Roadmap

Future enhancements: uniqueness scoring, duplicate token analysis, frequency divergence metrics for tougher edge cases.
