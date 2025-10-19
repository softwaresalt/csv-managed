# csv-managed

`csv-managed` is a Rust command-line utility for high‑performance exploration and transformation of CSV data at scale. It emphasizes streaming, typed operations, and reproducible workflows via schema (`.schema`) and index (`.idx`) files.

## Implemented Features

| Area | Description |
|------|-------------|
| Delimiters | Input/output: comma, tab (`tab`), pipe (\|), semicolon (`;`), or any single ASCII char; output delimiter can differ (`--output-delimiter`). |
| Schema inference (`probe`) | Full scan or sampled (`--sample-rows`) inference: String, Integer, Float, Boolean, Date, DateTime → stored as JSON `.schema`. |
| Probe mappings | Optional `--mapping` flag prints schema mapping templates and injects default `name_mapping` entries (lowercase_snake_case) into the generated schema. |
| Schema authoring (`schema`) | Manually declare columns with data types and optional output names, writing a `.schema` file without inferring from data. |
| Value normalization | Schema-level `replace` arrays (value/replacement pairs) convert legacy tokens (e.g. `Y/N`, `Pending`) into canonical values before parsing, filtering, stats, or verification. Generate empty templates during probing with `--replace`. |
| Replacement command (`fix`) | Stream existing files through schema-defined replacements to materialize cleaned outputs while preserving column order and delimiter/encoding choices. |
| Schema listing (`columns`) | Print a table of column positions, names, types, and optional output names from a schema file. |
| Typed parsing | Integer, float, boolean normalization (`true/false`, `t/f`, `yes/no`, `y/n`, `1/0`); multi-format dates & datetimes. |
| Indexing (`index`) | B-Tree style composite key index (byte offsets) for fast ascending iteration. |
| Indexed sorted reads | `process --sort` uses matching index for streaming ascending order. |
| Index variants | Build multiple named sort specifications per index file (`--spec`); select with `--index-variant`. |
| In‑memory multi-column sort | Stable sort with mixed asc/desc columns (fallback when no index). |
| Filtering | Operators: `= != > >= < <= contains startswith endswith`; type-aware; repeat for AND chaining. |
| Column projection | Include explicit columns (`-C/--columns`). |
| Column exclusion | Omit columns via `--exclude-columns` (after inclusion). |
| Derived columns | Expression engine (evalexpr) arithmetic, comparisons, boolean logic, string ops; reference by header or positional alias `cN`. |
| Row numbers | Optional 1-based `row_number` via `--row-numbers`. |

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
4. Extended expression library (regex, date math, if/else, null coalescing, aggregates).
5. Parallelism (rayon) for sort/filter phases.
6. Configurable reader options (quote char, flexible header detection, escapes).
7. Boolean token customization via schema.
8. Index versioning & migration utilities.
9. Cardinality & histogram statistics for planning.
10. Feature flags to reduce binary size.
11. Streaming approximations (e.g. median via P² algorithm).
12. Documentation site + generated man pages.
13. Benchmark harness & published results.
14. Machine-readable error codes.
15. Native decimal (fixed precision) & rounding strategies.

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
./target/release/csv-managed.exe fix -i ./data/orders.csv -o ./data/orders_clean.csv --schema ./data/orders.schema
# 5. Summary statistics
./target/release/csv-managed.exe stats -i ./data/orders.csv -m ./data/orders.schema
# 6. Frequency counts (top 10)
./target/release/csv-managed.exe frequency -i ./data/orders.csv -m ./data/orders.schema --top 10
# 7. Preview first 15 rows
./target/release/csv-managed.exe preview -i ./data/orders.csv --rows 15
# 8. Join customers with orders
./target/release/csv-managed.exe join --left ./data/orders.csv --right ./data/customers.csv --left-key customer_id --right-key id --type inner -o joined.csv
# 9. Append monthly extracts
./target/release/csv-managed.exe append -i jan.csv -i feb.csv -i mar.csv -m orders.schema -o q1.csv
# 10. Verify integrity
./target/release/csv-managed.exe verify -m orders.schema -i q1.csv
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
  --replace
```

cmd.exe:

```batch
./target/release/csv-managed.exe probe ^
  -i ./data/orders.csv ^
  -m ./data/orders.schema ^
  --delimiter tab ^
  --sample-rows 0 ^
  --replace
```

Omit `--replace` if you prefer to keep the generated schema free of placeholder `replace` arrays.

Legacy flag note: all commands continue to accept the historical `--meta`/`--left-meta`/`--right-meta` aliases for compatibility, but new examples use the canonical `--schema` options.

### schema

Create a `.schema` JSON file from explicit column definitions.

| Flag | Description |
|------|-------------|
| `-o, --output <FILE>` | Destination schema file. |
| `-c, --column <SPEC>` | Repeatable `name:type` definitions; commas allowed per flag. Append `->New Name` to assign an output name. |
| `--replace <COLUMN=FROM->TO>` | Repeatable value replacement directives applied before type parsing (e.g. `--replace status=Pending->Open`). |

PowerShell:

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

### fix

Apply schema-defined replacements to a CSV file, streaming the result to a new file or stdout.

| Flag | Description |
|------|-------------|
| `-i, --input <FILE>` | Input CSV file to transform. |
| `-o, --output <FILE>` | Destination CSV file (stdout if omitted). |
| `-m, --schema <FILE>` | Schema file containing `replace` mappings. |
| `--delimiter <VAL>` | Input delimiter. |
| `--output-delimiter <VAL>` | Output delimiter (defaults to input delimiter). |
| `--input-encoding <ENC>` | Character encoding of the input file (defaults to utf-8). |
| `--output-encoding <ENC>` | Character encoding for the output file/stdout (defaults to utf-8). |

PowerShell:

```powershell
./target/release/csv-managed.exe fix `
  -i ./data/orders.csv `
  -o ./data/orders_clean.csv `
  --schema ./data/orders.schema
```

cmd.exe:

```batch
./target/release/csv-managed.exe fix ^
  -i ./data/orders.csv ^
  -o ./data/orders_clean.csv ^
  --schema ./data/orders.schema
```

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

### preview

Display first N rows in an elastic table.

| Flag | Description |
|------|-------------|
| `-i, --input <FILE>` | Input file. |
| `--rows <N>` | Number of data rows (default 10). |
| `--delimiter <VAL>` | Input delimiter. |

### stats

Summary statistics for numeric columns.

| Flag | Description |
|------|-------------|
| `-i, --input <FILE or ->` | Input file or `-` (stdin; requires schema). |
| `-m, --schema <FILE>` | Schema file (recommended). |
| `-C, --columns <LIST>` | Restrict to listed numeric columns. |
| `--delimiter <VAL>` | Input delimiter. |
| `--limit <N>` | Scan at most N rows (0 = all). |

### frequency

Distinct value counts per column.

| Flag | Description |
|------|-------------|
| `-i, --input <FILE or ->` | Input file or stdin (`-`; schema required). |
| `-m, --schema <FILE>` | Optional schema. |
| `-C, --columns <LIST>` | Limit to listed columns. |
| `--delimiter <VAL>` | Input delimiter. |
| `--top <N>` | Limit to top N values (0 = all). |

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

### Data Types

| Type | Examples | Notes |
|------|----------|-------|
| String | any UTF‑8 | Normalized header names usable in expressions. |
| Integer | `42`, `-7` | 64-bit signed. |
| Float | `3.14`, `2` | f64; integers accepted. |
| Boolean | `true/false`, `t/f`, `yes/no`, `y/n`, `1/0` | Parsing flexible; output format selectable. |
| Date | `2024-08-01`, `08/01/2024`, `01/08/2024` | Canonical output `YYYY-MM-DD`. |
| DateTime | `2024-08-01T13:45:00`, `2024-08-01 13:45` | Naive (no timezone). |

Pending: Decimal, Time-only.

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

- Indexed sort avoids loading all rows into memory.
- Early filtering cuts sort & derive workload.
- Derived expressions evaluated per emitted row—keep them lean.
- Median requires storing column values (potential memory impact for huge numeric columns).

### Error Handling

- Rich `anyhow` contexts (I/O, parsing, evaluation, schema, index).
- Fast failure on unknown columns, invalid expressions, header/schema mismatches.
- Invalid UTF‑8 rows error (never silently skipped).

### Logging

Set `RUST_LOG=csv_managed=debug` (or `info`) for insight into phases (index use, inference, filtering).

### Testing

```bash
cargo test
```

Integration tests cover probe, index, process (filters, derives, sort, delimiters). Additional tests planned for joins, stats, frequency.

### Contributing

1. Fork & branch (`feat/<name>`).
2. Add tests (unit + integration) for new behavior.
3. Run `cargo fmt && cargo clippy && cargo test` before PR.
4. Update README (move items from roadmap when implemented).

### License

See `LICENSE`.

### Support

Open issues for bugs, enhancements, or documentation gaps. Pull requests welcome.
