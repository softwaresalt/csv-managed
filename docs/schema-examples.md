# Schema Command Examples

The examples below demonstrate common `schema` command permutations using the bundled Big 5 player statistics dataset located under `tests/data`.

## Probe Without Writing A File

Inspect a handful of rows and review inferred samples, format hints, and override status directly in the console:

```powershell
csv-managed schema probe -i tests/data/big_5_players_stats_2023_2024.csv --sample-rows 5
```

**Output highlights:**

- Table columns: raw name, inferred type, rename (if any), override flag, sample value, and format hint.
- Footer summarises how many rows were scanned and whether any decode errors were encountered.

## Infer With Mapping And Replace Templates

Generate a schema file populated with snake_case renames and empty replacement arrays so you can fill in value substitutions later:

```powershell
csv-managed schema infer --mapping --replace-template `
    -i tests/data/big_5_players_stats_2023_2024.csv `
    -o tmp/big5_inferred.schema --sample-rows 0
```

This command performs a full scan (`--sample-rows 0`) before writing `tmp/big5_inferred.schema`.

## Override Inferred Types

Force specific column types while still injecting automatic renames for downstream processing:

```powershell
csv-managed schema infer --mapping `
    --override Performance_Gls:integer `
    --override "Per 90 Minutes_Gls:string" `
    -i tests/data/big_5_players_stats_2023_2024.csv `
    -o tmp/big5_overrides.schema --sample-rows 10
```

The resulting schema keeps inferred types for all other columns, but `Performance_Gls` becomes `integer` and `Per 90 Minutes_Gls` becomes `string` with the rename `per_90_minutes_gls`.

## Freeze Layout With A Snapshot

Capture the current probe table rendering and fail future runs if the layout changes unexpectedly:

```powershell
csv-managed schema probe -i tests/data/big_5_players_stats_2023_2024.csv --sample-rows 5 --snapshot tmp/big5_probe.snap
```

On first execution the file `tmp/big5_probe.snap` is created. Subsequent executions compare the live output to the stored snapshot and return a non-zero exit code if the formatting differs.
