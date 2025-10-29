# Boolean Formatting & Table Output

## Boolean Output Modes

Use `--boolean-format` on `process` to select canonical rendering:

- `original` (raw parsed token case preserved if already canonical)
- `true-false`
- `one-zero`

Examples:

```powershell
csv-managed process -i orders.csv -m orders-schema.yml --boolean-format one-zero -C shipped_flag -o shipped.csv
csv-managed process -i orders.csv -m orders-schema.yml --boolean-format true-false --table -C shipped_flag
```

## Parsing Flexibility

Accepted (case-insensitive): `true false t f yes no y n 1 0`. Mixed forms normalize internally; output formatting only affects emitted value.

## Table Rendering

`--table` renders an elastic ASCII table when writing to stdout (omit `-o`). Combine with `--limit` for quick inspection.

```powershell
csv-managed process -i orders.csv --preview --limit 15  # implicit table preview
csv-managed process -i orders.csv --table --limit 10 -C order_id,status
```

## Pipeline Considerations

- `--table` should be avoided when piping into downstream CSV consumers (it outputs formatted text, not CSV).
- Use `--preview` for quick header/value inspection—cannot combine with `-o`.

## Best Practices

1. Pick `one-zero` for ML-friendly binary features.
2. Use `true-false` for human-readable audit exports.
3. Avoid table mode in machine pipelines; write CSV to file or stdout without `--table`.

## Roadmap

Potential future: configurable custom boolean token sets via schema metadata.
