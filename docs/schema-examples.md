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

## Majority-Based Inference Logic (Examples)

The inference engine selects a column's datatype via majority voting across sampled (or full scan) non-empty values. Below are illustrative scenarios you can reproduce by crafting small CSV snippets.

### 1. Integer Majority Wins

`scores.csv`:

```text
score
10
22
5
7
14
3
9
11
8
6
12
4
```

All 12 values parse cleanly as `Integer` (and implicitly as `Float`). Integer holds a 100% majority so the inferred type is `Integer`.

```powershell
csv-managed schema probe -i scores.csv --sample-rows 0
```

### 2. Mixed Integer & Float Promotes to Float

`mixed_numeric.csv`:

```text
amount
10
22.5
5
7.75
14
3.10
9.0
11
8
```

Votes: Integer (6), Float (9). Integer does not exceed 50% of parsed values, so Float (plurality) is selected.

### 3. Decimal Scale Majority Promotes to decimal(p,s)

`precise.csv`:

```text
measurement
1.2345
2.1000
3.0000
4.9999
5.1234
```

Each value fits scale 4; the engine infers `decimal(18,4)` (precision shown may vary based on max digits encountered). If a minority value had 5 decimals, Decimal would lose majority and fall back to Float (plurality) unless overridden.

### 4. Plurality Fallback With Dates & DateTimes

`temporal_mixed.csv`:

```text
stamp
2024-01-01
2024-01-02 04:30:00
2024-01-03
2024-01-04 17:15:00
misc
```

Votes: Date (2), DateTime (2), String (1). No majority; specificity ordering prefers the simpler canonical subset when exact tie—result: `Date` (because Date tokens can be parsed uniformly while DateTime adds optional time components without majority advantage). If an additional DateTime line were added to break the tie, DateTime would win.

### 5. Currency Promotion With Symbol Threshold

`prices.csv`:

```text
amount
$12.00
14
15
```

All three values satisfy currency parsing (0 or 2 decimal places) and one-third of the sample bears a currency symbol. Inference now promotes the column to `Currency` before integer/float evaluation, preventing price columns like this from being labelled `Integer`.

### 6. Using Overrides to Stabilize Edge Columns

If a mostly-numeric ID column contains a few non-numeric placeholders (e.g., `NA`, `#N/A`) those rows can spoil majority. Provide replacements or force type:

```powershell
csv-managed schema infer -i ids.csv -o ids-schema.yml --override id:Integer --sample-rows 0
```

### 7. Upcoming NA Token Normalization

Planned enhancement (backlog) will allow treating `NA`, `N/A`, `#NA`, `#N/A` as empty votes so they no longer dilute numeric or temporal majorities. Until then, consider adding explicit replacements (`replace:` arrays) for such tokens.

### Quick Tips

- Increase `--sample-rows` beyond the default (2000) when early rows are skewed.
- Keep at least roughly one-third of sampled rows with currency symbols if you want automatic Currency detection; otherwise supply overrides.
- Use `--snapshot` after confirming inference to lock the layout & types.
- Inspect the probe table's sample and format hints for confirmation before persisting a schema.

## Freeze Layout With A Snapshot

Capture the current probe table rendering and fail future runs if the layout changes unexpectedly:

```powershell
csv-managed schema probe -i tests/data/big_5_players_stats_2023_2024.csv --sample-rows 5 --snapshot tmp/big5_probe.snap
```

On first execution the file `tmp/big5_probe.snap` is created. Subsequent executions compare the live output to the stored snapshot and return a non-zero exit code if the formatting differs.

## Normalize Datatypes With `datatype_mappings`

Schema files can declare transformation steps that run before value replacements or final type parsing. Below is the same example expressed in YAML (preferred) converting ISO‑8601 timestamps into bare dates and rounding verbose decimals to four places:

```yaml
columns:
  - name: order_ts
    datatype: Date
    datatype_mappings:
      - from: String
        to: DateTime
      - from: DateTime
        to: Date
  - name: amount
    datatype: Float
    datatype_mappings:
      - from: String
        to: Float
        strategy: round
        options:
          scale: 4
```

The `process`, `append`, `stats`, and `schema verify` commands automatically apply these mappings unless you opt out with `--skip-mappings`. Use `--apply-mappings` to enforce them when chaining custom workflows.

### Datatype Mapping Reference

Key rules and capabilities when authoring `datatype_mappings`:

1. Capitalization: Data types in production schema files should be capitalized (`String`, `Integer`, `Float`, `Boolean`, `Date`, `DateTime`, `Time`, `Guid`, `Currency`). All YAML examples below follow this convention.
2. Ordering: Mappings are applied in declaration order, top to bottom. Each mapping consumes the previous output.
3. Options: A mapping may include an `options` object to guide parsing or rendering (e.g., a custom datetime `format`).
4. Strategies: Supported `strategy` values (case-insensitive) by context: `round` (numeric, including Currency), `truncate` (numeric to Integer or Currency scale adjustment), `trim` / `lowercase` / `uppercase` (String→String). Rounding uses a `scale` in `options` (defaults to `4` for Float; Currency requires an explicit allowed scale of `2` or `4`).
5. Error handling: If any mapping step fails (e.g., a datetime parse), the entire row fails verification for that column.

#### Custom String → DateTime Parsing

If your timestamp includes a trailing `Z` (UTC designator) or other formatting not covered by the built‑in fallbacks, supply an explicit chrono format string:

```yaml
name: ordered_raw
datatype: Date
rename: ordered_at
datatype_mappings:
  - from: String
    to: DateTime
    options:
      format: "%Y-%m-%dT%H:%M:%SZ"
  - from: DateTime
    to: Date
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

## Currency Mappings And Validation

The `Currency` datatype enforces a fixed scale of either 2 or 4 decimal places. Parsing accepts common symbols (`$`, `€`, `£`, `¥`), thousands separators (`,`), and negative formats (leading `-` or parentheses). Any value not matching an allowed scale is rejected during verification.

### Examples

#### 1. String → Currency (Round to 2 decimal places)

```yaml
columns:
  - name: gross_amount_raw
    datatype: Currency
    datatype_mappings:
      - from: String
        to: Currency
        strategy: round
        options:
        scale: 2
```

#### 2. String → Currency (Truncate to 4 decimal places)

```yaml
columns:
  - name: fx_rate_raw
    datatype: Currency
    datatype_mappings:
      - from: String
        to: Currency
        strategy: truncate
        options:
        scale: 4
```

#### 3. Float → Currency (Direct conversion)

If upstream processing produced a `Float` and you need a canonical currency with 2 decimals, you can map directly.

```yaml
columns:
  - name: net_amount
    datatype: Currency
    datatype_mappings:
      - from: Float
        to: Currency
        strategy: round
        options:
        scale: 2
```

### Strategy Notes

- `round`: Midpoint rounding is applied away from zero to enforce the target scale.
- `truncate`: Removes excess fractional digits without rounding (e.g., `123.456789` with scale 4 becomes `123.4567`).
- Scale must be explicitly supplied for Currency mappings (`2` or `4`). Omitting scale yields a validation error.

### Validation & Stats

Currency columns participate in numeric aggregations (min, max, mean, std dev) and frequency displays. The original scale is preserved for consistent formatting during output.

### Error Modes

Common failure conditions for Currency parsing:

1. Invalid symbol placement (e.g., `123$` instead of `$123`).
2. Unsupported scale (`123.456` → 3 decimals is rejected).
3. Mixed thousands separators and spaces (e.g., `$1, 234.00`).
4. Parentheses with a leading minus (`-(123.45)`), double negative.
5. Trailing or leading stray characters (`$123.45USD`).

Design your mappings to sanitize upstream data before conversion when necessary.
