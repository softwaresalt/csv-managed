# Snapshots vs Schema Verification

Two complementary quality gates: layout regression (snapshots) and data conformity (verification).

## Comparison

| Aspect | Snapshot (`--snapshot`) | Verification (`schema verify`) |
|--------|-------------------------|--------------------------------|
| Purpose | Guard inference layout & formatting | Enforce per-cell datatype & replacements |
| Scope | Headers + inferred types + samples | Entire row values (streaming) |
| Artifact | Snapshot text file | Reports only (optional invalid samples) |
| Trigger | Run on `schema probe` / `schema infer` | Dedicated subcommand |
| Failure | Layout/type drift | Invalid cell parse / replacement mismatch |
| Typical Use | CI regression lock | Pipeline/data quality gate |
| Performance | Fast (sample) | Scales to full file size |

## Workflow

```powershell
# Lock inference
csv-managed schema infer -i data.csv -o data-schema.yml --snapshot infer.snap --sample-rows 0
# Later data quality
csv-managed schema verify -m data-schema.yml -i new_extract.csv --report-invalid:detail:summary 10
```

## When To Refresh Snapshot

After intentional header rename, datatype override, or inference heuristic upgrade. Review diff; commit updated file.

## Reporting Tiers (Verification)

| Flag | Row Samples | Column Summary | Notes |
|------|-------------|----------------|-------|
| `--report-invalid` | ✖ | ✔ | Fast overview |
| `--report-invalid:detail` | ✔ | ✖ | Highlight samples |
| `--report-invalid:detail:summary` | ✔ | ✔ | Full context |
| `--report-invalid:detail 5` | Limited | ✖ | Cap samples |
| `--report-invalid:detail:summary 5` | Limited | ✔ | Sample cap only |

Base mode (no flag) logs a count; non-zero exit code signals any invalid cell.

## Best Practices

1. Keep snapshot row sample modest to avoid noisy diffs.
2. Pair verify in CI before append / stats / indexing.
3. Use `replace` + mappings to clean data before verification.
4. Do not treat snapshots as correctness—they only lock presentation & inference.

## Troubleshooting

| Symptom | Likely Cause | Fix |
|---------|--------------|-----|
| Snapshot mismatch | Header or type drift | Accept intentional change (re-run) or investigate regression |
| Many invalid decimals | Scale/precision mismatch | Adjust schema (decimal(p,s)) or mapping strategies |
| Currency failing | Unsupported scale or symbol noise | Clean tokens, enforce allowed scale (2 or 4) |
| Boolean parsed as String | Unclassified token like `maybe` | Add replacements or override |

## Roadmap

Future: structured JSON snapshot export, selective diffing, composite key validation integration.
