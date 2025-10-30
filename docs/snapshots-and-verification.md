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

## The Role of Snapshots in an Auditable Pipeline

In any data workflow where trust and repeatability are critical, it's not enough to know *what* you did; you must also prove *what you did it to*. This is the core principle behind an auditable pipeline, and it highlights the distinct, complementary roles of schema files and snapshot files.

* **The Schema File: The "Intent"**
    The schema file (`-schema.yml`) is the **rulebook**. It declares your intent: how to rename columns, what data types to enforce, and which value transformations to apply. It answers the question: "What rules should be applied to the data?"

* **The Snapshot File: The "Fingerprint"**
    The snapshot file is the **cryptographic proof** of the input data's structure at a specific point in time. It contains a hash of the column headers and their inferred types. It answers the question: "Was the raw input data structurally identical to what I expected?"

Together, they form a verifiable chain of evidence. The snapshot's role in an audit trail is to:

1. **Detect Upstream Data Drift:** Its primary function is to act as a guard. If an upstream system changes a source file's structure (renaming, reordering, or adding columns), snapshot validation will fail immediately. This prevents the silent processing of corrupted or unexpected data and creates a clear, auditable failure event.

2. **Provide a Point-in-Time "Seal":** For any given pipeline run, the snapshot acts as a seal of authenticity on the input. An audit package can include the input data, the schema (the rules), and the snapshot (the proof of structure), proving that the rules were applied to a known and verified data structure.

3. **Enable Reproducibility:** To reproduce a past result for an audit, you must recreate the exact starting conditions. The snapshot is essential for this. It allows you to first verify that the historical input data has the exact structure that was processed originally before re-running the transformation.

4. **Formalize Change Control:** When a structural change to the input data is intentional, the act of generating a new snapshot becomes a formal, auditable event. Committing the new snapshot to version control creates a permanent record of when and why the accepted structure of the source data was changed.

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
