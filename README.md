# csv-managed

`csv-managed` is a Rust command-line utility for high‑performance exploration and transformation of CSV data at scale. It emphasizes streaming, typed operations, and reproducible workflows via schema (`.schema`) and index (`.idx`) files.

## Implemented Features

| Area | Description |
|------|-------------|
| Delimiters & Encodings | Read/write comma, tab, pipe, semicolon, or any single ASCII delimiter; independent `--input-encoding` / `--output-encoding`; stdin/stdout streaming (`-`). See: [process](#process), [index](#index). |
| Schema Discovery (probe / infer) | Fast sample (`--sample-rows`) or full scan detection of String, Integer, Float, Boolean, Date, DateTime, Time, Guid; optional mapping & replace scaffolds (`--mapping`, `--replace-template`); overrides via `--override`. See: [schema](#schema). |
| Manual Schema Authoring | Inline column specs (`-c name:type->Alias`), value replacements (`--replace column=value->new`), persisted to `.schema`. See: [schema](#schema). |
| Snapshot Regression | `--snapshot <file>` for `schema probe` / `schema infer` writes or validates golden layout & inferred types; guards against formatting/inference drift. See: [Snapshot vs Schema Verify](#snapshot-vs-schema-verify). |
| Column Listing | `schema columns` renders column positions, types, and aliases derived from schema mapping. See: [schema columns](#schema-columns). |
| Value Normalization | Per-column `replace` arrays applied before parsing; flexible boolean token parsing with selectable output format (`process --boolean-format`). See: [schema](#schema), [process](#process). |
| Datatype Transformations | Schema-driven `datatype_mappings` chains convert and standardize values (string→datetime→date, float rounding, string casing) before replacements; toggle via `process --apply-mappings` / `--skip-mappings`. See: [schema](#schema), [process](#process). |
| Indexing | Multi-variant B-tree index files with mixed asc/desc columns; named specs (`--spec name=col:asc,...`) and combo expansion (`--combo`) for prefix/direction permutations. See: [index](#index). |
| Sort & Stream Processing | `process` selects best index variant (longest matching prefix) or falls back to stable in-memory multi-column sort while streaming transformations. See: [process](#process). |
| Filtering & Projection | Typed comparison filters (`= != > >= < <= contains startswith endswith`), multi-flag AND semantics; Evalexpr predicates (`--filter-expr`) with temporal helpers; column include/exclude; row limiting; optional 1-based row numbers. See: [process](#process), [Expression Reference](#expression-reference). |
| Temporal Expression Helpers | Functions like `date_diff_days`, `datetime_format`, `time_diff_seconds` usable in derives and `--filter-expr`. See: [process](#process), [Expression Reference](#expression-reference). |
| Derived Columns | Evalexpr-based expressions referencing header names or positional aliases (`cN`); arithmetic, string, conditional, temporal operations. See: [process](#process), [Expression Reference](#expression-reference). |
| Append | Concatenate multiple inputs with header (and optional schema) validation, enforcing consistent types pre-merge. See: [append](#append). |
| Verification | `schema verify` streams each row against declared types; rich reports via `--report-invalid:detail[:summary] [LIMIT]`. See: [schema](#schema), [Snapshot vs Schema Verify](#snapshot-vs-schema-verify). |
| Statistics & Frequency | `stats` computes count, mean, median, min, max, std dev for numeric & temporal columns; `--frequency` distinct counts with optional `--top`; filters apply prior to aggregation. See: [stats](#stats). |
| Preview & Table Rendering | `process --preview` elastic table for quick inspection (defaults `--limit` to 10); `process --table` formatted output when streaming to stdout. See: [process](#process). |
| Joins (engine) | Hash-join engine retained for upcoming streaming pipelines; CLI command temporarily disabled while v1.6.0 join workflows are redesigned. |
| Installation & Tooling | `install` convenience wrapper around `cargo install`; tag-based release workflow; logging via `RUST_LOG`. See: [install](#install). |
| Streaming & Memory Efficiency | Forward-only iteration for verify, stats, filtering, projection, and indexed sorted reads; minimizes heap usage for large files. See: [process](#process), [schema](#schema). |
| Error Reporting & Diagnostics | Contextual errors (I/O, parsing, schema mismatch, expression eval); highlighted invalid cells; snapshot mismatch failures surface layout drifts early. See: [schema](#schema), [process](#process). |

### Mini Derived & Filter Expression Cheat Sheets

#### Derived Expression Patterns

| Pattern | Example | Description |
|---------|---------|-------------|
| Header reference | `total_with_tax=amount*1.0825` | Multiply numeric column values. |
| Positional alias | `margin=c5-c3` | Use `cN` alias (0-based). |
| Conditional flag | `high_value=if(amount>1000,1,0)` | 1/0 indicator via `if(cond, then, else)`. |
| Date math | `ship_eta=date_add(ordered_at,2)` | Add days to a date column. |
| Date diff | `ship_lag=date_diff_days(shipped_at,ordered_at)` | Days between two dates. |
| Time diff | `window=time_diff_seconds(end_time,start_time)` | Seconds between two times. |
| Boolean normalization | `is_shipped=if(status="shipped",true,false)` | Emit canonical booleans. |
| String concat | `channel_tag=concat(channel,"-",region)` | Join string columns. |
| Guid passthrough | `id_copy=id` | Duplicate a Guid column. |
| Row number | `row_index=row_number` | Sequential number (with `--row-numbers`). |

#### Filter vs Filter-Expr Cheat Sheet

| Aspect | --filter | --filter-expr |
|--------|----------|---------------|
| Syntax style | Simple operator tokens | Full Evalexpr expression |
| Supported operators | = != > >= < <= contains startswith endswith | arithmetic(+ - * / %), logic(AND OR), functions(if(), concat(), date_*, time_*) |
| Type awareness | Typed parsing per column | Evaluates on parsed typed values |
| Temporal helpers | Compare canonical values | date_diff_days, date_add, time_diff_seconds, datetime_format, etc. |
| Boolean logic | Repeat flag = AND chain | logical operators (AND, OR) or if(...) nesting |
| String literals | Bare or quoted if spaces | Must use double quotes (outer shell may use single) |
| Column reference | Header name | Header name or positional alias cN |
| Row number | Provided when --row-numbers | Variable row_number when --row-numbers |
| Example | --filter "status = shipped" | --filter-expr 'if(amount>1000 && status="shipped", true, false)' |
| Temporal example | --filter "ordered_at >= 2024-01-01" | --filter-expr 'date_diff_days(shipped_at, ordered_at) >= 2' |
| Complex gating | Multiple --filter flags | Single rich conditional expression |

Common `--filter-expr` snippets:

```text
date_diff_days(shipped_at, ordered_at) > 1
concat(channel, "-", region) = "web-US"
if(amount * 1.0825 > 500, 1, 0)
time_diff_seconds(end_time, start_time) >= 3600
```

### Expression Reference

Unified reference for derived column expressions, filter vs filter-expr usage, temporal helpers, common pitfalls, and a combined example.

#### 1. Derived Columns (Recap)

Use `--derive name=expression`. Expressions may reference:

* Header names (normalized after schema mapping)
* Positional aliases `cN` (0-based, so `c0` is first data column)
* Functions (see Temporal Helpers below)
* `row_number` (only when `--row-numbers` enabled)

#### 2. Filter vs Filter-Expr (Recap)

Two parallel mechanisms:

* `--filter` provides concise typed comparisons (auto-parsed per datatype; AND chaining across repeats).
* `--filter-expr` evaluates a full Evalexpr expression after parsing typed values (supports arithmetic, string, conditional, temporal helpers, boolean logic).

Mix them freely; all are combined with AND semantics overall (i.e. row must satisfy every filter and every filter-expr that evaluates true).

#### 3. Temporal Helpers (Full List)

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

All helpers accept canonical strings (`YYYY-MM-DD`, `YYYY-MM-DD HH:MM:SS`, `HH:MM:SS`). Time arguments accept `HH:MM`. Fractional numeric offsets are truncated.

#### 4. Common Pitfalls

| Pitfall | Guidance |
|---------|----------|
| Quoting (PowerShell) | Wrap the whole expression in single quotes; use double quotes for string literals inside: `'channel="web"'`. |
| Quoting (cmd.exe) | Escape inner quotes: `"web"`. |
| Positional alias indexing | `c0` is first column, not `c1`; verify header order after mapping. |
| Mixed filter logic | Multiple --filter flags AND together; to OR conditions use --filter-expr with (a OR b). |
| Row number usage | `row_number` available only if `--row-numbers` was set before derives/filters execute. |
| Temporal comparisons | Prefer helpers (e.g. `date_diff_days`) over manual string comparison for correctness across formats. |
| Transform vs replace ordering | `datatype_mappings` run first, followed by schema `replace` mappings, then typed parsing & expressions; design expressions based on the fully normalized values. |
| Boolean output format | `--boolean-format` affects derived boolean rendering; logic still works with internal canonical bool. |
| Performance & median | Median and large numeric derives may retain many values; limit columns or avoid heavy expressions for huge files. |
| Using snapshots | Snapshots guard inference output only; they do not validate expression correctness. |

#### 5. Combined Filtering Example

Example mixing concise filters and one complex temporal expression:

```powershell
./target/release/csv-managed.exe process \
  -i ./data/orders.csv \
  -m ./data/orders.schema \
  --filter "status = shipped" \
  --filter "amount >= 100" \
  --filter-expr 'date_diff_days(shipped_at, ordered_at) >= 2 && (region = "US" || region = "CA")' \
  --derive 'ship_lag_days=date_diff_days(shipped_at, ordered_at)' \
  --row-numbers \
  -C order_id,ordered_at,shipped_at,ship_lag_days,amount,status,region,row_number \
  --limit 25
```

#### 6. Quick Expression Validation Tip

Start with a narrower column selection (`-C`) and a small `--limit` to confirm derived outputs before removing the limit for full processing.

#### 7. Function Index (Alphabetical)

Helper functions usable in `--derive` and `--filter-expr` (temporal & formatting):

`date_add`, `date_diff_days`, `date_format`, `date_sub`, `datetime_add_seconds`, `datetime_diff_seconds`, `datetime_format`, `datetime_to_date`, `datetime_to_time`, `time_add_seconds`, `time_diff_seconds`

#### 8. Debug Tip

Set an environment variable to increase internal logging verbosity:

PowerShell:

```powershell
$env:RUST_LOG='csv_managed=debug'
```

cmd.exe:

```batch
set RUST_LOG=csv_managed=debug
```

Future enhancement: a debug mode may emit expression parse/normalize traces (e.g., tokenization, type coercions). When added, they will appear at `debug` level tagged with `expr:` prefixes. This placeholder documents intended usage; if absent, no expression AST logging is currently implemented.

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
./target/release/csv-managed.exe schema infer -i ./data/orders.csv -o ./data/orders.schema --sample-rows 0
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
./target/release/csv-managed.exe process -i ./data/orders.csv --preview --limit 15
# 8. Append monthly extracts
./target/release/csv-managed.exe append -i jan.csv -i feb.csv -i mar.csv -m orders.schema -o q1.csv
# 9. Verify integrity (summary default)
./target/release/csv-managed.exe schema verify -m orders.schema -i q1.csv
#     Investigate failures with highlighted samples (optional limit)
./target/release/csv-managed.exe schema verify -m orders.schema -i orders_invalid.csv --report-invalid:detail:summary 5
```

For more advanced derived column and filtering patterns (bucketing, temporal calculations, chained logic), see `docs/expressions.md`.

## Command Reference

Detailed `--help` output for every command is mirrored in `docs/cli-help.md` for quick reference.

### schema

Define schemas manually or discover them via `probe` / `infer`; verify datasets against a saved schema and optionally enforce value replacements.

| Subcommand / Flag | Description |
|-------------------|-------------|
| `schema probe` | Display inferred columns and types in a console table (no file written). |
| `schema infer` | Infer and optionally persist a `.schema` file (`-o/--output`). |
| `schema verify` | Stream-validate one or more files against a schema (`-m/--schema`). |
| `-i, --input <FILE>` | Input CSV for `probe` or `infer`. Repeat `-i` for multiple inputs in `verify`. |
| `-o, --output <FILE>` | Destination schema file (alias `--schema` retained for compatibility). |
| `-m, --schema <FILE>` | Schema file to use with `verify` (or as destination alias with `infer`). |
| `-c, --column <SPEC>` | Manual column definitions (`name:type`, or `name:type->Alias`). Repeatable / comma list. |
| `--replace <SPEC>` | Value replacement directive (`column=value->replacement`) for manual schema authoring. |
| `--sample-rows <N>` | Rows to sample during inference (`0` = full scan). |
| `--delimiter <VAL>` | Override input delimiter (`comma`, `tab`, `semicolon`, `pipe`, or single ASCII). |
| `--input-encoding <ENC>` | Character encoding of input (defaults `utf-8`). |
| `--mapping` | Emit column mapping templates (aliases) to stdout when probing/infering. |
| `--replace-template` | Inject empty `replace` arrays per column when inferring. |
| `--override <SPEC>` | Force specific inferred types (`amount:Float`, `id:Integer`). Repeatable. |
| `--snapshot <PATH>` | Capture/compare probe or infer output against a golden snapshot. Writes if missing, fails on drift; see [Snapshot Internals](#snapshot-internals). |
| `--report-invalid[:detail[:summary]] [LIMIT]` | (verify) Add row samples (`:detail`) and/or column summary (`:summary`); optional LIMIT caps sample rows. |

Behavior notes:

* `schema probe` renders an elastic table of inferred columns plus sample-based hints; footer indicates scan scope and any decoding skips.
* `schema infer` shares all probe options and adds persistence, mapping templates, and optional replace scaffolding.
* `schema verify` streams every row, applying replacements before type parsing; failures can produce ANSI-highlighted samples and column summaries.
* `--snapshot` applies to `probe` and `infer`, guarding the textual layout & inference heuristics (see [Snapshot Internals](#snapshot-internals) and [Snapshot vs Schema Verify](#snapshot-vs-schema-verify)).

More end-to-end examples: [`docs/schema-examples.md`](docs/schema-examples.md).

PowerShell (inference mode):

```powershell
./target/release/csv-managed.exe schema infer `
  -i ./data/orders.csv `
  -o ./data/orders.schema `
  --delimiter tab `
  --sample-rows 0 `
  --mapping `
  --replace-template
```

PowerShell (explicit columns with replacements):

```powershell
./target/release/csv-managed.exe schema `
  -o ./schemas/orders.schema `
  -c id:integer->Identifier `
  -c customer_id:integer->Customer ID,order_date:date,amount:float,status:string `
  --replace status=Pending->Open `
  --replace "status=Closed (Legacy)->Closed"
```

cmd.exe:

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

### Snapshot Internals

Snapshot files captured by `schema probe --snapshot` or `schema infer --snapshot` now contain structured diagnostics to make regression reviews easier:

* **Header+Type Hash** – a SHA-256 digest that locks the column ordering and inferred datatypes. Any change to headers or datatypes produces a new hash even if table formatting stays the same.
* **Datatype Map** – an expanded list of every column name paired with the inferred datatype icon, making it simple to diff type changes without scanning the table output.
* **Column Summaries** – for each column, the snapshot records how many non-empty values were seen during sampling, how many empty rows appeared, and a small histogram (up to five representative values plus an “others” bucket). This mirrors the sampling scope shown in the footer so you can spot drift in categorical distributions or sparsity.

When a snapshot mismatch occurs, these sections highlight exactly which aspect changed—structure, type inference, or observed value distribution—before you decide whether to refresh the snapshot.

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
| `--preview` | Render a preview table on stdout (defaults `--limit` to 10; cannot be combined with `--output`). |
| `--table` | Render as formatted table when streaming to stdout (ignored when writing to a file). |
| (see Expression Reference) | Advanced derived, filter, and temporal helper syntax. |

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

### Snapshot vs Schema Verify

The `--snapshot` flag (used with `schema probe` or `schema infer`) and the `schema verify` subcommand serve **complementary but distinct** purposes:

| Aspect | Snapshot (`schema probe --snapshot` / `schema infer --snapshot`) | Schema verify (`schema verify`) |
|--------|------------------------------------------------------------------|---------------------------------|
| Primary goal | Guard against unintended changes in probe/infer formatting or inference heuristics (layout, ordering, inferred types) | Enforce that actual CSV row values conform to a declared schema (types, headers, replacements) |
| Domain | Developer regression / output stability | Data quality / contractual correctness |
| Data scanned | Headers + optional sampled rows (based on `--sample-rows`) | Entire file(s), streaming every row |
| Artifact | A snapshot text file (golden layout); created if missing, compared if present | No artifact on success; optional ANSI-highlighted report on failure |
| Validation granularity | Whole rendered output string (byte/line comparison) | Per‑cell parsing & typed normalization |
| Failure cause | Rendered output differs from saved snapshot | Any cell cannot be parsed/mapped to its declared datatype |
| Typical CI use | Lock down formatting & inference behavior so docs/tests stay stable | Block ingestion of malformed or schema‑incompatible data |
| Performance profile | Very fast (sample + render) | Potentially heavy for large files; optimized via streaming |
| Update workflow | Rerun with `--snapshot` intentionally to refresh after accepted changes | Update schema file separately as data definitions evolve |

#### When to Use Which

Use a snapshot when you want to ensure the *presentation and inference logic* of schema discovery has not drifted (e.g., after refactors or heuristic tweaks). Use `schema verify` when validating real datasets prior to append, stats, indexing, joins, or downstream ML pipelines.

#### Example Workflow

```powershell
# 1. Infer schema and create/update snapshot of inference layout
./target/release/csv-managed.exe schema infer -i data.csv -o data.schema --snapshot infer.snap --sample-rows 0

# 2. Commit both data.schema and infer.snap

# 3. Later, validate new extracts against the frozen schema
./target/release/csv-managed.exe schema verify -m data.schema -i new_extract.csv --report-invalid:detail:summary 25
```

If inference heuristics or display formatting changes intentionally, refresh the snapshot:

```powershell
./target/release/csv-managed.exe schema probe -i data.csv --snapshot infer.snap --sample-rows 10
```

This will overwrite (if removed first) or fail (if differing) to prompt a conscious review. Keep snapshots small by combining them with modest `--sample-rows` values—full scans are unnecessary for layout regression.

#### Summary

*Snapshot = regression guard on inferred schema presentation.*  
*Verify = runtime enforcement of data correctness against a schema.*

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
| (see Expression Reference) | Extended filter / temporal helper functions. |

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

The hash-join engine remains part of the codebase, but the standalone `join` subcommand has been withdrawn from the CLI while we redesign a streaming-friendly workflow for v1.6.0. Existing scripts should transition to `process`-first pipelines (filters, derives, preview, append) until the new join interface lands. Follow the roadmap in `[.todos/plan-v1.6.0.md](.todos/plan-v1.6.0.md)` for progress updates on the pipeline-oriented join strategy.

### install

### schema columns

List schema columns and their data types in a formatted table.

| Flag | Description |
|------|-------------|
| `-m, --schema <FILE>` | Schema file describing the columns to list. |

PowerShell:

```powershell
./target/release/csv-managed.exe schema columns `
  --schema ./data/orders.schema
```

cmd.exe:

```batch
./target/release/csv-managed.exe schema columns ^
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

> See the [Expression Reference](#expression-reference) for temporal helper usage (date/time arithmetic & formatting), boolean output formatting considerations, and quoting rules affecting String, Date, DateTime, Time parsing in derived expressions and filters.

### Datatype Mappings

`datatype_mappings` let you declare an ordered chain of conversions that run **before** value replacements and final type parsing. Author them inside each column object in the schema file.

Key points:

* Capitalization: Use capitalized data types (`String`, `Integer`, `Float`, `Boolean`, `Date`, `DateTime`, `Time`, `Guid`) in production schema files.
* Order matters: Each mapping consumes the previous output; declare from raw → intermediate → final.
* Strategies: `round` (numeric), `trim` / `lowercase` / `uppercase` (String→String), `truncate` (Float→Integer). Rounding scale defaults to `4` unless `options.scale` is provided.
* Options: Provide an `options` object for format guidance (e.g. a datetime `format`) or numeric rounding scale.
* Failure: Any mapping parse error invalidates the row for that column during `schema verify`.

Example converting an ISO‑8601 timestamp with trailing `Z` to a date and rounding a decimal:

```json
{
  "name": "ordered_raw",
  "datatype": "Date",
  "rename": "ordered_at",
  "datatype_mappings": [
    { "from": "String", "to": "DateTime", "options": { "format": "%Y-%m-%dT%H:%M:%SZ" } },
    { "from": "DateTime", "to": "Date" }
  ]
},
{
  "name": "amount_raw",
  "datatype": "Float",
  "rename": "amount",
  "datatype_mappings": [
    { "from": "String", "to": "Float", "strategy": "round", "options": { "scale": 4 } }
  ]
}
```

Built‑in fallback DateTime formats (used when no explicit `options.format` is specified):

```text
%Y-%m-%d %H:%M:%S
%Y-%m-%dT%H:%M:%S
%d/%m/%Y %H:%M:%S
%m/%d/%Y %H:%M:%S
%Y-%m-%d %H:%M
%Y-%m-%dT%H:%M
```

To parse timestamps with a trailing `Z`, offsets, or fractional seconds, supply a matching `options.format` (e.g., `%Y-%m-%dT%H:%M:%SZ`, `%Y-%m-%dT%H:%M:%S%.f`).

Common chrono tokens:

| Token | Meaning |
|-------|---------|
| `%Y`  | 4‑digit year |
| `%m`  | Month (01–12) |
| `%d`  | Day of month (01–31) |
| `%H`  | Hour (00–23) |
| `%M`  | Minute (00–59) |
| `%S`  | Second (00–60) |
| `%f`  | Fractional seconds (nanoseconds) |

Validation flow:

1. Raw value ingested.
2. `datatype_mappings` chain executes.
3. Value replacements apply.
4. Final parsing validates against the declared column `datatype`.

See extended examples in `docs/schema-examples.md`.

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

Integration tests cover schema inference, index, process (filters, derives, sort, delimiters). Additional tests planned for joins and stats frequency scenarios.

### Contributing

1. Fork & branch (`feat/<name>`).
2. Add tests (unit + integration) for new behavior.
3. Run `cargo fmt && cargo clippy && cargo test` before PR.
4. Update README (move items from roadmap when implemented).

### License

See `LICENSE`.

### Support

Open issues for bugs, enhancements, or documentation gaps. Pull requests welcome.
