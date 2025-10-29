# Encoding Normalization Pipelines

Normalize legacy encodings (e.g. Windows-1252) to UTF-8 early to prevent downstream parse failures.

## Pattern

```powershell
Get-Content .\tmp\legacy_windows1252.csv | \
  .\target\release\csv-managed.exe process -i - --input-encoding windows-1252 --schema .\schemas\layout-schema.yml | \
  .\target\release\csv-managed.exe stats -i - --schema .\schemas\layout-schema.yml -C amount
```

## Guidelines

1. Apply encoding normalization before filters, derives, or stats.
2. Keep header shape unchanged during normalization for schema reuse.
3. If multiple heterogeneous sources: normalize each then append.

## Common Issues

| Symptom | Cause | Fix |
|---------|-------|-----|
| Replacement character `�` | Wrong `--input-encoding` | Re-run with correct encoding or convert upstream |
| Unexpected inference fallback to String | Hidden decode errors | Inspect logs (`RUST_LOG=info`) and normalize first |

## Output Encoding

Use `--output-encoding utf-8` to enforce canonical output when writing to file:

```powershell
csv-managed process -i legacy.csv --input-encoding windows-1252 --schema layout-schema.yml -o normalized.csv --output-encoding utf-8
```

## Best Practices

- Commit normalized UTF-8; avoid mixing encodings in version control.
- Pair with `schema verify` after normalization to catch latent invalid tokens.

## Roadmap

Future: automatic detection hints and multi-file batch normalization command.
