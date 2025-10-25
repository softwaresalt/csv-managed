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
    -o tmp/big5_inferred-schema.yml --sample-rows 0
```

This command performs a full scan (`--sample-rows 0`) before writing `tmp/big5_inferred-schema.yml`.

## Override Inferred Types

Force specific column types while still injecting automatic renames for downstream processing:

```powershell
csv-managed schema infer --mapping `
    --override Performance_Gls:integer `
    --override "Per 90 Minutes_Gls:string" `
    -i tests/data/big_5_players_stats_2023_2024.csv `
    -o tmp/big5_overrides-schema.yml --sample-rows 10
```

The resulting schema keeps inferred types for all other columns, but `Performance_Gls` becomes `integer` and `Per 90 Minutes_Gls` becomes `string` with the rename `per_90_minutes_gls`.

## Freeze Layout With A Snapshot

Capture the current probe table rendering and fail future runs if the layout changes unexpectedly:

```powershell
csv-managed schema probe -i tests/data/big_5_players_stats_2023_2024.csv --sample-rows 5 --snapshot tmp/big5_probe.snap
```

On first execution the file `tmp/big5_probe.snap` is created. Subsequent executions compare the live output to the stored snapshot and return a non-zero exit code if the formatting differs.

## Normalize Datatypes With `datatype_mappings`

Schema files can now declare transformation steps that run before value replacements or type parsing. For example, the snippet below converts ISO 8601 timestamps into bare dates and rounds verbose decimals to four places:

```json
{
    "columns": [
        {
            "name": "order_ts",
            "datatype": "date",
            "datatype_mappings": [
                { "from": "string", "to": "datetime" },
                { "from": "datetime", "to": "date" }
            ]
        },
        {
            "name": "amount",
            "datatype": "float",
            "datatype_mappings": [
                {
                    "from": "string",
                    "to": "float",
                    "strategy": "round",
                    "options": { "scale": 4 }
                }
            ]
        }
    ]
}
```

The `process`, `append`, `stats`, and `schema verify` commands automatically apply these mappings unless you opt out with `--skip-mappings`. Use `--apply-mappings` to enforce them when chaining custom workflows.

### Datatype Mapping Reference

Key rules and capabilities when authoring `datatype_mappings`:

1. Capitalization: Data types in production schema files should be capitalized (`String`, `Integer`, `Float`, `Boolean`, `Date`, `DateTime`, `Time`, `Guid`). The examples above use lowercase for brevity; prefer capitalized forms for clarity and consistency.
2. Ordering: Mappings are applied in declaration order, top to bottom. Each mapping consumes the previous output.
3. Options: A mapping may include an `options` object to guide parsing or rendering (e.g., a custom datetime `format`).
4. Strategies: Supported `strategy` values (case-insensitive) by context: `round` (numeric), `trim` / `lowercase` / `uppercase` (String→String), `truncate` (Float→Integer). Rounding uses a `scale` in `options` (defaults to `4`).
5. Error handling: If any mapping step fails (e.g., a datetime parse), the entire row fails verification for that column.

#### Custom String → DateTime Parsing

If your timestamp includes a trailing `Z` (UTC designator) or other formatting not covered by the built‑in fallbacks, supply an explicit chrono format string:

```json
{
    "name": "ordered_raw",
    "datatype": "Date",
    "rename": "ordered_at",
    "datatype_mappings": [
        { "from": "String", "to": "DateTime", "options": { "format": "%Y-%m-%dT%H:%M:%SZ" } },
        { "from": "DateTime", "to": "Date" }
    ]
}
```

#### Built-In DateTime Fallback Formats

When no `options.format` is provided for a `String` → `DateTime` mapping, the parser attempts these formats in order:

```text
%Y-%m-%d %H:%M:%S
%Y-%m-%dT%H:%M:%S
%d/%m/%Y %H:%M:%S
%m/%d/%Y %H:%M:%S
%Y-%m-%d %H:%M
%Y-%m-%dT%H:%M
```

To parse timestamps that include timezone markers (`Z`, offsets) or fractional seconds, you must provide a matching `options.format` string.

#### Common Chrono Format Tokens

| Token | Meaning |
|-------|---------|
| `%Y`  | 4‑digit year |
| `%m`  | Month (01–12) |
| `%d`  | Day of month (01–31) |
| `%H`  | Hour (00–23) |
| `%M`  | Minute (00–59) |
| `%S`  | Second (00–60) |
| `%f`  | Fractional seconds (nanoseconds) |
| `%z`  | Offset like `+0000` (requires explicit format & non‑naive parsing) |
| `%Z`  | Timezone name/abbrev (not applied to naive conversions) |

For ISO‑8601 with a trailing `Z` (e.g. `2024-04-01T08:30:00Z`), include the literal `Z` at the end of the pattern (`%Y-%m-%dT%H:%M:%SZ`).

#### Validation Flow

1. Raw value ingested.
2. `datatype_mappings` chain executes.
3. Value replacements (if any) apply.
4. Final typed parsing validates against the column's declared `datatype`.

Design mappings so the last step produces a representation parsable as the target datatype.
