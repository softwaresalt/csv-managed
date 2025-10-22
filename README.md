# csv-managed

`csv-managed` is a Rust command-line utility for high‑performance exploration and transformation of CSV data at scale. It emphasizes streaming, typed operations, and reproducible workflows via schema (`.schema`) and index (`.idx`) files.

## Implemented Features

| Area | Description |
|------|-------------|
| Delimiters & encodings | Read/write comma, tab, pipe, semicolon, or any ASCII delimiter; override output delimiter; stream via stdin/stdout (`-`) with explicit `--input-encoding` / `--output-encoding`. |
| Schema inference (`probe`) | Sample or full-scan detection of String, Integer, Float, Boolean, Date, DateTime, Time, and Guid columns; optional `--mapping` and `--replace` templates saved to `.schema`. |
| Schema authoring & listing | `schema` builds manual definitions with renames and replacements; `columns` prints column positions, names, types, and output aliases. |
| Value normalization | Schema `replace` arrays and boolean normalization convert legacy tokens before typed operations; `process --boolean-format` controls emitted true/false representation. |
| Indexing (`index`) | Build B-tree index files with multiple named variants, mixed asc/desc columns, and combo expansion for shared prefixes. |
| Sorting (`process`) | Streams through matching index variants for ordered reads and falls back to stable multi-column in-memory sort when needed. |
| Filtering & projection | Type-aware filters (`= != > >= < <= contains startswith endswith`), evalexpr filters (`--filter-expr`) with temporal helpers, column inclusion/exclusion, row limits, and optional 1-based row numbers. |
| Derived columns | Evalexpr-powered expressions provide arithmetic, comparison, conditional, and string operations referencing headers or positional aliases (`cN`). |
| Append (`append`) | Concatenate files with header validation and optional schema enforcement to guarantee consistent types before merging. |
| Verification (`verify`) | Validates each file against the schema; default output is a column summary. `--report-invalid:detail[:summary] [LIMIT]` adds ANSI-highlighted row samples with optional sample limits. |
| Stats & frequency | `stats` streams count/mean/median/min/max/stddev per numeric (Integer/Float) and temporal (Date/DateTime/Time) columns; temporal std dev includes units (`days` or `seconds`). `--filter`/`--filter-expr` limit the rows considered and also apply to `stats --frequency`, which reports distinct value counts with optional top-N cap. |
| Preview & table output | `preview` shows the first N rows as an elastic table; `process --table` renders transformed output as a table on stdout. |
| Joins (`join`) | Hash join supports inner, left, right, and full outer joins with schema-driven typing and replacement normalization for keys. |
| Installation | `install` wraps `cargo install` with convenience flags (`--locked`, `--force`, `--root`, `--version`) and matches the release workflow. |

cmd.exe:

```batch
./target/release/csv-managed.exe process ^
  -i ./data/orders.csv ^
  -m ./data/orders.schema ^
  -x ./data/orders.idx ^
  --index-variant default ^
  --sort order_date:asc,customer_id:asc ^
  --filter "status = shipped" ^
  --filter "amount >= 100" ^
  --derive "total_with_tax=1" ^
  --derive "channel=\"online\"" ^
  -C order_id,customer_id,amount,total_with_tax ^
  --exclude-columns internal_flag ^
  --row-numbers ^
  --boolean-format one-zero ^
  --output-delimiter pipe
```

If `--index-variant` is omitted, `process` automatically chooses the variant that covers the longest prefix of the requested `--sort` columns and directions.
When no indexed variant matches the requested sort signature, the command falls back to an in-memory stable sort while continuing to stream rows wherever possible.

## Installation

```bash
cargo build --release
```

Binary (Windows): `target\release\csv-managed.exe`

Install from crates.io:

```bash
cargo install csv-managed
```

Install locally from the workspace (useful when developing):

```bash
cargo install --path .
```

After building, the CLI can re-run installation on the current machine:

```powershell
./target/release/csv-managed.exe install --locked
```

The helper wraps `cargo install csv-managed` and accepts `--version`, `--force`, `--locked`, and `--root` pass-through options.

> Release automation: push a tag like `v0.1.0` and provide a `CRATES_IO_TOKEN` repository secret; the GitHub Actions release workflow will build archives and execute `cargo publish --locked` automatically.

Logging examples:

```powershell
$env:RUST_LOG='info'
```

```batch
set RUST_LOG=info
```

## Quick Start

```powershell
# 1. Infer schema
./target/release/csv-managed.exe probe -i ./data/orders.csv -m ./data/orders.schema --sample-rows 0
# 2. Build index (optional for sorted reads)
./target/release/csv-managed.exe index -i ./data/orders.csv -o ./data/orders.idx --spec default=order_date:asc,customer_id:asc --spec recent=order_date:desc --schema ./data/orders.schema
# 3. Process with filters / derives / sort
./target/release/csv-managed.exe process -i ./data/orders.csv -m ./data/orders.schema -x ./data/orders.idx --index-variant default --sort order_date:asc,customer_id:asc --filter "status = shipped" --derive 'total_with_tax=amount*1.0825' --row-numbers -o ./data/orders_filtered.csv
# 4. Normalize legacy tokens via schema replacements
./target/release/csv-managed.exe process -i ./data/orders.csv -o ./data/orders_clean.csv --schema ./data/orders.schema
# 5. Summary statistics
./target/release/csv-managed.exe stats -i ./data/orders.csv -m ./data/orders.schema
# 5b. Temporal summary statistics
./target/release/csv-managed.exe stats -i ./tests/data/stats_temporal.csv -m ./tests/data/stats_temporal.schema --columns ordered_at --columns ordered_at_ts --columns ship_time
# 6. Frequency counts (top 10)
./target/release/csv-managed.exe stats -i ./data/orders.csv -m ./data/orders.schema --frequency --top 10
# 7. Preview first 15 rows
./target/release/csv-managed.exe preview -i ./data/orders.csv --rows 15
# 8. Join customers with orders
./target/release/csv-managed.exe join --left ./data/orders.csv --right ./data/customers.csv --left-key customer_id --right-key id --type inner -o joined.csv
# 9. Append monthly extracts
./target/release/csv-managed.exe append -i jan.csv -i feb.csv -i mar.csv -m orders.schema -o q1.csv
# 10. Verify integrity (summary default)
./target/release/csv-managed.exe verify -m orders.schema -i q1.csv
#     Investigate failures with highlighted samples (optional limit)
./target/release/csv-managed.exe verify -m orders.schema -i orders_invalid.csv --report-invalid:detail:summary 5
```

## Command Reference

Detailed `--help` output for every command is mirrored in `docs/cli-help.md` for quick reference.

### probe

Infer column types and produce a `.schema` JSON file.

| Flag | Description |
|------|-------------|
| `-i, --input <FILE>` | Input CSV file. |
| `-m, --schema <FILE>` | Output schema file. |
| `--sample-rows <N>` | Sample size (`0` = full scan). |
| `--delimiter <VAL>` | Input delimiter. |
| `--mapping` | Insert default lowercase_with_underscores `name_mapping` aliases into the `.schema` file and print templates to stdout. |
| `--replace` | Emit empty `replace` arrays for each column as a template for future value substitutions. |

PowerShell:

```powershell
./target/release/csv-managed.exe probe `
  -i ./data/orders.csv `
  -m ./data/orders.schema `
  --delimiter tab `
  --sample-rows 0 `
  ```

```powershell
./target/release/csv-managed.exe schema `
  -o ./schemas/orders.schema `
  -c id:integer->Identifier `
  -c customer_id:integer->Customer ID,order_date:date,amount:float,status:string ^
  --replace status=Pending->Open ^
  --replace status=Closed (Legacy)->Closed
```

cmd.exe:
Use `--replace` to normalize legacy tokens or synonyms before validation and processing; each entry populates the column's `replace` array in the schema file.

```batch
./target/release/csv-managed.exe schema ^
  -o ./schemas/orders.schema ^
  -c id:integer->Identifier ^
  -c customer_id:integer->Customer ID,order_date:date,amount:float,status:string
```

### index

Build a B-Tree index for specified key columns (ascending order optimization).

| Flag | Description |
|------|-------------|
| `-i, --input <FILE>` | Input CSV file. |
| `-o, --index <FILE>` | Output `.idx` file. |
| `-C, --columns <LIST>` | Legacy single ascending index (comma list). Superseded by `--spec`. |
| `--spec <SPEC>` | Repeatable: `name=col_a:asc,col_b:desc` or `col_a:asc`. Builds named variants per index file. |
| `--combo <SPEC>` | Generate prefix combinations with optional direction branches using `\|`, e.g. `geo=date:asc\|desc,customer:asc`. |
| `-m, --schema <FILE>` | Optional schema file. |
| `--limit <N>` | Stop after N rows (partial index). |
| `--delimiter <VAL>` | Input delimiter. |

PowerShell:

```powershell
./target/release/csv-managed.exe index `
  -i ./data/orders.csv `
  -o ./data/orders.idx `
  --spec default=order_date:asc,customer_id:asc `
  --spec recent=order_date:desc `
  -m ./data/orders.schema
```

cmd.exe:

```batch
./target/release/csv-managed.exe index ^
  -i ./data/orders.csv ^
  -o ./data/orders.idx ^
  --spec default=order_date:asc,customer_id:asc ^
  --spec recent=order_date:desc ^
  -m ./data/orders.schema
```

`--spec` accepts comma-separated `column:direction` tokens. Prefix with `name=` to label the variant (e.g. `fast=col_a:asc,col_b:desc`). When omitted, the variant is anonymous but still usable for automatic matching.

### process

Transform pipeline: sort, filter, derive, project, exclude, boolean formatting, row numbers, delimiter changes, optional table output.

| Flag | Description |
|------|-------------|
| `-i, --input <FILE>` | Input CSV (required). |
| `-o, --output <FILE>` | Output file (stdout if omitted). |
| `-m, --schema <FILE>` | Schema file. |
| `-x, --index <FILE>` | Index file for accelerated sort matching asc/desc directives. |
| `--index-variant <NAME>` | Pin to a named variant stored in the index file (requires matching `--sort`). |
| `--sort <SPEC>` | Repeatable: `column[:asc or :desc]`. Comma list or multiple uses. |
| `-C, --columns <LIST>` | Inclusion list (repeatable). |
| `--exclude-columns <LIST>` | Exclusion list (repeatable). |
| `--derive <name=expr>` | Derived column (repeatable). |
| `--filter <expr>` | Filter expression (repeatable; AND). |
| `--filter-expr <EXPR>` | Evalexpr-based filter evaluated per row; supports temporal helpers (`date_add`, `date_diff_days`, `time_diff_seconds`, etc.). Use double-quoted string literals for constants (e.g., `"06:00:00"`). |
| `--row-numbers` | Prepend `row_number`. |
| `--limit <N>` | Emit at most N rows. |
| `--delimiter <VAL>` | Input delimiter. |
| `--output-delimiter <VAL>` | Output delimiter override. |
| `--boolean-format <FORMAT>` | Normalize boolean output. Formats: `original`, `true-false`, `one-zero`. |
| `--table` | Render as formatted table (stdout only; incompatible with `--output`). |

PowerShell:

```powershell
./target/release/csv-managed.exe process `
  -i ./data/orders.csv `
  -m ./data/orders.schema `
  -x ./data/orders.idx `
  --index-variant default `
  --sort order_date:asc,customer_id:asc `
  --filter "status = shipped" `
  --filter "amount >= 100" `
  --derive 'total_with_tax=amount*1.0825' `
  --derive 'channel="online"' `
  -C order_id,customer_id,amount,total_with_tax `
  --exclude-columns internal_flag `
  --row-numbers `
  --boolean-format one-zero `
  --output-delimiter pipe
```

cmd.exe:

```batch
./target/release/csv-managed.exe process ^
  -i ./data/orders.csv ^
  -m ./data/orders.schema ^
  -x ./data/orders.idx ^
  --index-variant default ^
  --sort order_date:asc,customer_id:asc ^
  --filter "status = shipped" ^
  --filter "amount >= 100" ^
  --derive "total_with_tax=amount*1.0825" ^
  --derive "channel=\"online\"" ^
  -C order_id,customer_id,amount,total_with_tax ^
  --exclude-columns internal_flag ^
  --row-numbers ^
  --boolean-format one-zero ^
  --output-delimiter pipe
```

If `--index-variant` is omitted, `process` automatically chooses the variant that covers the longest prefix of the requested `--sort` columns and directions.

Schema-driven replacements defined in the `.schema` file are always applied before parsing, so `process` can clean and transform data in a single pass.

### append

Append multiple CSV files into a single output. Ensures consistent headers (baseline or schema enforced).

| Flag | Description |
|------|-------------|
| `-i, --input <FILE>` | Repeatable input files (first defines header). |
| `-o, --output <FILE>` | Output file (stdout if omitted). |
| `-m, --schema <FILE>` | Optional schema for strict validation. |
| `--delimiter <VAL>` | Delimiter for all inputs. |

Example:

```powershell
./target/release/csv-managed.exe append -i jan.csv -i feb.csv -i mar.csv -m orders.schema -o q1.csv
```

### verify

Validate one or more CSV files against a schema definition.

| Flag | Description |
|------|-------------|
| `-m, --schema <FILE>` | Schema file. |
| `-i, --input <FILE>` | Repeatable input files. |
| `--delimiter <VAL>` | Input delimiter. |
| `--report-invalid[:detail[:summary]] [LIMIT]` | Summarize invalid columns by default; append `:detail` for row samples, `:detail:summary` for both, and optionally add LIMIT to cap sample rows. |

### preview

Display first N rows in an elastic table.

| Flag | Description |
|------|-------------|
| `-i, --input <FILE>` | Input file. |
| `--rows <N>` | Number of data rows (default 10). |
| `--delimiter <VAL>` | Input delimiter. |

### stats

Summary statistics for numeric and temporal columns.

Supported types:

* Numeric: Integer, Float
* Temporal: Date, DateTime, Time

Temporal values are internally converted to numeric metrics for aggregation:

* Date => days from Common Era (CE)
* DateTime => epoch seconds (UTC naive)
* Time => seconds from midnight

They are rendered back to canonical forms; standard deviation for Date reports `days` and for DateTime/Time reports `seconds`.

| Flag | Description |
|------|-------------|
| `-i, --input <FILE or ->` | Input file or `-` (stdin; requires schema). |
| `-m, --schema <FILE>` | Schema file (recommended). |
| `-C, --columns <LIST>` | Restrict to listed columns (defaults to numeric & temporal columns, or all columns when `--frequency` is used). |
| `--delimiter <VAL>` | Input delimiter. |
| `--frequency` | Emit distinct value counts instead of summary statistics. |
| `--top <N>` | Limit to the top N values per column when `--frequency` is used (0 = all). |
| `--limit <N>` | Scan at most N rows (0 = all). |

#### Temporal stats example

Given a temporal schema file:

```jsonc
{
  "columns": [
    { "name": "id", "datatype": "Integer" },
    { "name": "ordered_at", "datatype": "Date" },
    { "name": "ordered_at_ts", "datatype": "DateTime" },
    { "name": "shipped_at", "datatype": "Date" },
    { "name": "shipped_at_ts", "datatype": "DateTime" },
    { "name": "ship_time", "datatype": "Time" },
    { "name": "status", "datatype": "String" }
  ]
}
```

Run stats over temporal columns:

```powershell
./target/release/csv-managed.exe stats -i ./data/orders_temporal.csv -m ./data/orders_temporal.schema \
  --columns ordered_at --columns ordered_at_ts --columns ship_time
```

Sample output (elastic table formatting):

```text
| column         | count | min                | max                | mean                | median              | std_dev        |
| ordered_at     | 4     | 2024-01-01         | 2024-02-10         | 2024-01-31          | 2024-01-06          | 15.56 days     |
| ordered_at_ts  | 4     | 2024-01-01 04:45:00| 2024-02-10 14:00:00| 2024-01-30 17:03:45 | 2024-01-06 05:57:30 | 1345678 seconds|
| ship_time      | 4     | 06:00:00           | 16:30:00           | 09:37:30            | 08:00:00            | 12810 seconds  |
```

Mean and median for Time represent the central tendency of seconds-from-midnight values, rendered back into `HH:MM:SS`.

Apply filters to restrict the rows included in the calculation:

```powershell
./target/release/csv-managed.exe stats -i ./data/stats_schema.csv -m ./data/stats_schema.schema \
  --columns quantity --filter "status=good"
```

`--filter` accepts the same column comparisons as `process --filter`. For complex predicates, repeat `--filter` or add `--filter-expr` for Evalexpr-based expressions. Filters apply to both summary statistics and `--frequency` output.

#### Frequency counts (--frequency)

`stats --frequency` reports distinct value counts per column. By default, every column is included; use `-C/--columns` to target a subset. Combine with `--top` to cap the number of values displayed per column (0 = all).

Example combining `--frequency` with filters over the Big 5 dataset:

```powershell
./target/release/csv-managed.exe stats `
  -i ./tests/data/big_5_players_stats_2023_2024.csv `
  --frequency `
  -C Squad `
  --filter "Player=Max Aarons"
```

Sample output (elastic table formatting):

```text
| column | value       | count | percent |
| Squad  | Bournemouth | 1     | 100.00% |
```

### join

Join two CSV files on one or more key columns.

| Flag | Description |
|------|-------------|
| `--left <FILE or ->` | Left input file or stdin (`-`; requires left schema). |
| `--right <FILE>` | Right input file (file path). |
| `-o, --output <FILE>` | Output file (stdout if omitted). |
| `--left-key <COLS>` | Comma-separated left key columns. |
| `--right-key <COLS>` | Comma-separated right key columns. |
| `--type <inner/left/right/full>` | Join type (default inner). |
| `--left-schema <FILE>` | Left schema (required if left is stdin). |
| `--right-schema <FILE>` | Right schema. |
| `--delimiter <VAL>` | Input delimiter. |

Example:

```powershell
./target/release/csv-managed.exe join `
  --left orders.csv `
  --right customers.csv `
  --left-key customer_id `
  --right-key id `
  --type left `
  -o orders_with_customers.csv
```

### install

### columns

List schema columns and their data types in a formatted table.

| Flag | Description |
|------|-------------|
| `-m, --schema <FILE>` | Schema file describing the columns to list. |

PowerShell:

```powershell
./target/release/csv-managed.exe columns `
  --schema ./data/orders.schema
```

cmd.exe:

```batch
./target/release/csv-managed.exe columns ^
  --schema ./data/orders.schema
```

Wrapper around `cargo install csv-managed` with a friendlier interface.

| Flag | Description |
|------|-------------|
| `--version <SEMVER>` | Install a specific published version. |
| `--force` | Reinstall even if already installed. |
| `--locked` | Pass `--locked` to respect `Cargo.lock`. |
| `--root <DIR>` | Target an alternate installation directory. |

Example:

```powershell
./target/release/csv-managed.exe install --locked
```

#### Derived Column Expression Notes

| Access Pattern | Example | Meaning |
|----------------|---------|---------|
| Normalized header | `total_with_tax=amount*1.0825` | Uses inferred numeric type. |
| Positional alias | `c3*1.1` | Fourth column (0-based). |
| String literal (PS) | `'tag="promo"'` | Single quotes wrap inner quotes. |
| String literal (cmd) | `"tag=\"promo\""` | Escaped inner quotes. |
| Row number | `row_index=row_number` | Available when `--row-numbers` enabled. |

#### Filter Syntax Examples

PowerShell:

```powershell
--filter "status = shipped" --filter "amount >= 100" --filter "customer_id startswith 1"
```

Mixed operators:

```powershell
--filter "order_date >= 2024-01-01" --filter "description contains urgent" --filter "region != US"
```

### Temporal Expression Helpers

`--filter-expr` and derived column expressions can use built-in helpers for manipulating dates, times, and datetimes:

| Function | Description |
|----------|-------------|
| `date_add(date, days)` / `date_sub(date, days)` | Shift a date forward or backward by whole days. |
| `date_diff_days(end, start)` | Difference in days between two dates (can be negative). |
| `date_format(date, "%d %b %Y")` | Render a date with a custom chrono-compatible format string. |
| `datetime_add_seconds(ts, seconds)` | Shift a datetime by an offset in seconds. |
| `datetime_diff_seconds(end, start)` | Difference between datetimes in seconds. |
| `datetime_to_date(ts)` / `datetime_to_time(ts)` | Extract date or time portions from a datetime. |
| `datetime_format(ts, "%Y-%m-%dT%H:%M")` | Custom formatting for datetimes. |
| `time_add_seconds(time, seconds)` | Shift an `HH:MM[:SS]` time of day by seconds. |
| `time_diff_seconds(end, start)` | Difference between two times (seconds). |

All helpers accept and return canonical strings (e.g., `YYYY-MM-DD`, `YYYY-MM-DD HH:MM:SS`, `HH:MM:SS`). Time arguments also accept `HH:MM` shorthand. Integer offsets accept either integers or floats (fractional parts truncated).

PowerShell example:

```powershell
./target/release/csv-managed.exe process `
  -i ./data/orders.csv `
  -m ./data/orders.schema `
  --filter-expr "date_diff_days(shipped_at, ordered_at) >= 1" `
  --derive 'ship_eta=date_add(ordered_at, 2)' `
  --derive 'ship_window=time_diff_seconds(ship_time, "06:00:00")' `
  --columns id,ordered_at,shipped_at,ship_eta,ship_window `
  --limit 5
```

String literals in expressions must use double quotes (`""`) to distinguish them from column identifiers; the surrounding CLI quoting can use single quotes (recommended on PowerShell) or escaped double quotes as needed by the shell.

### Data Types

| Type | Examples | Notes |
|------|----------|-------|
| String | any UTF‑8 | Normalized header names usable in expressions. |
| Integer | `42`, `-7` | 64-bit signed. |
| Float | `3.14`, `2` | f64; integers accepted. |
| Boolean | `true/false`, `t/f`, `yes/no`, `y/n`, `1/0` | Parsing flexible; output format selectable. |
| Date | `2024-08-01`, `08/01/2024`, `01/08/2024` | Canonical output `YYYY-MM-DD`. |
| DateTime | `2024-08-01T13:45:00`, `2024-08-01 13:45` | Naive (no timezone). |
| Time | `06:00:00`, `14:30`, `08:01:30` | Canonical output `HH:MM:SS`; inference accepts `HH:MM[:SS]`. |
| Guid | `550e8400-e29b-41d4-a716-446655440000`, `550E8400E29B41D4A716446655440000` | Case-insensitive; accepts hyphenated or 32-hex representations. |

Future work: Decimal, Currency.

### Stdin/Stdout Usage

Use `-` for streaming input where supported. Example:

```powershell
Get-Content orders.csv | ./target/release/csv-managed.exe stats -i - -m orders.schema
```

### Boolean Formatting Examples

```powershell
./target/release/csv-managed.exe process -i orders.csv -m orders.schema --boolean-format one-zero -C shipped_flag -o shipped.csv
./target/release/csv-managed.exe process -i orders.csv -m orders.schema --boolean-format true-false --table -C shipped_flag
```

### Table Output

`--table` renders transformation results as an elastic-width ASCII table (stdout only; cannot combine with `--output`).

### Index Strategy

Index stores byte offsets keyed by concatenated column values. A single `.idx` can hold multiple named variants, each with its own mix of ascending/descending columns. `process` picks the variant that best matches the requested `--sort` signature or you can force one via `--index-variant`. When no compatible variant exists, the command falls back to in-memory sorting.

### Performance Considerations

* Indexed sort avoids loading all rows into memory.
* Early filtering cuts sort & derive workload.
* Derived expressions evaluated per emitted row—keep them lean.
* Median requires storing column values (potential memory impact for huge numeric columns).

### Error Handling

* Rich `anyhow` contexts (I/O, parsing, evaluation, schema, index).
* Fast failure on unknown columns, invalid expressions, header/schema mismatches.
* Invalid UTF‑8 rows error (never silently skipped).

### Logging

Set `RUST_LOG=csv_managed=debug` (or `info`) for insight into phases (index use, inference, filtering).

### Testing

```bash
cargo test
```

Integration tests cover probe, index, process (filters, derives, sort, delimiters). Additional tests planned for joins and stats frequency scenarios.

### Contributing

1. Fork & branch (`feat/<name>`).
2. Add tests (unit + integration) for new behavior.
3. Run `cargo fmt && cargo clippy && cargo test` before PR.
4. Update README (move items from roadmap when implemented).

### License

See `LICENSE`.

### Support

Open issues for bugs, enhancements, or documentation gaps. Pull requests welcome.
