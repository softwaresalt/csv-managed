# Copilot Instructions for csv-managed

High-performance Rust CLI (edition 2024, v1.0.x) that streams, transforms, validates, indexes, and profiles large CSV/TSV datasets. Targets minimal memory footprint on Windows, macOS, and Linux.

## Project Layout

| Path | Purpose |
|------|---------|
| `src/lib.rs` | Crate root: module declarations, CLI dispatch via `run()`, operation timing |
| `src/cli.rs` | `clap` derive definitions: `Cli`, `Commands` enum, all `*Args` structs |
| `src/schema.rs` | `Schema` model, `ColumnType` enum (10 types), YAML load/save, type inference |
| `src/data.rs` | `Value` enum, typed parsers (bool, date, time, GUID, currency, decimal) |
| `src/process.rs` | `process` subcommand: sort, filter, project, derive, write |
| `src/index.rs` | B-tree index build, `CsvIndex`, `IndexDefinition`, `IndexVariant` |
| `src/filter.rs` | `FilterCondition`, `ComparisonOperator`, row-level filter parsing |
| `src/expr.rs` | `evalexpr`-based filter expressions |
| `src/stats.rs` | Summary statistics for numeric columns |
| `src/frequency.rs` | Distinct-value frequency counts |
| `src/append.rs` | Multi-file CSV concatenation with header validation |
| `src/verify.rs` | Schema verification against CSV files |
| `src/schema_cmd.rs` | Schema subcommand dispatch: probe, infer, verify, columns, manual create |
| `src/io_utils.rs` | Reader/writer construction, delimiter/encoding resolution, stdin/stdout |
| `src/derive.rs` | Derived column expressions (`name=expression`) |
| `src/rows.rs` | Row-level typed parsing and filter expression evaluation |
| `src/columns.rs` | Column listing from schema files |
| `src/join.rs` | Join two CSV files (inner, left, right, full) -- currently commented out |
| `src/table.rs` | ASCII table rendering for `--preview` and `--table` output |
| `src/install.rs` | Self-install via `cargo install` |
| `tests/*.rs` | Integration tests using `assert_cmd` + `predicates` |
| `tests/data/` | Fixture CSV and schema YAML files |
| `benches/` | Criterion benchmarks |
| `docs/` | ADRs, CLI help, operation guides |
| `specs/` | Feature specifications and task plans |

## Architecture Rules

* Stream CSV data row-by-row using `csv::Reader`. Never load entire files into memory.
* Each subcommand follows the pattern: public `execute(args: &XArgs) -> Result<()>` in its own module, dispatched from `lib.rs::run()`.
* All operations run inside `run_operation()`, which wraps execution with structured timing output (start, end, duration) and outcome logging.
* Use `anyhow::Result` and `anyhow::Context` for error propagation throughout the codebase. There is no custom error enum; all modules use `anyhow`.
* Log via `log` crate macros (`info!`, `debug!`, `error!`). The `env_logger` backend initializes once in `init_logging()`. Never `println!` from deep logic; bubble status up to the CLI layer.
* Exit codes: `0` success, `1` error.

## Key Dependencies and Usage

| Crate | Role |
|-------|------|
| `clap` (derive) | CLI argument parsing; all args in `cli.rs` |
| `csv` | Streaming CSV read/write with `QuoteStyle::Always` for output |
| `anyhow` | Error handling (`Result`, `Context`, `bail!`, `ensure!`) |
| `serde` + `serde_yaml` | Schema YAML serialization/deserialization |
| `chrono` | Date, time, datetime parsing (`NaiveDate`, `NaiveDateTime`, `NaiveTime`) |
| `rust_decimal` | Currency and fixed-precision decimal values with rounding |
| `encoding_rs` | Character encoding detection and transcoding |
| `evalexpr` | Runtime expression evaluation for `--filter-expr` and `--derive` |
| `sha2` | Content hashing for snapshot verification |
| `uuid` | GUID column type parsing |
| `similar` | Unified diff output for `schema infer --diff` |
| `itertools` | Iterator combinators |

## Schema and Type System

* Schemas are YAML files (`-schema.yml`) containing version, columns (name, alias, datatype, nullable, precision, format, replacements, mappings), and primary keys.
* The `ColumnType` enum has 10 variants: `String`, `Integer`, `Float`, `Boolean`, `Date`, `DateTime`, `Time`, `Currency`, `Decimal`, `Guid`.
* The `Value` enum mirrors `ColumnType` with parsed values plus `Null`.
* Currency values use `rust_decimal::Decimal` with allowed scales of 2 or 4.
* Fixed-precision decimals (`DecimalSpec`) carry precision and scale with configurable rounding strategies (truncate, round-half-up).
* Type inference samples a configurable number of rows (default 2000) and detects types via heuristic parsing order.
* Column renames and alias mappings resolve at ingestion boundary before any transforms.

## Coding Conventions Specific to This Codebase

* Rust edition 2024: use `let` chains and other stabilized features.
* Prefer `&str` and `Cow<'_, str>` over `String` cloning. Reuse scratch buffers in hot loops.
* Delimiter resolution is centralized in `io_utils`: extension-based auto-detection (`.csv` comma, `.tsv` tab) with manual override.
* The `-` path convention routes through stdin/stdout.
* `--preview` mode sets a default limit of 10 rows and renders an ASCII table.
* Use `parse_delimiter()` in `cli.rs` for any new delimiter arguments; it supports named values (`tab`, `pipe`, `semicolon`) and single ASCII characters.
* Keep function bodies under 100 lines; extract helpers for clarity.
* Add `//!` module-level Rustdoc to every source file describing scope and complexity.

## Testing Conventions

* Unit tests go in inline `#[cfg(test)]` modules within each source file.
* Integration tests go in `tests/*.rs`, using `assert_cmd::Command` and `predicates::str::contains` for CLI validation.
* Fixture helper pattern: `fn fixture_path(name: &str) -> PathBuf` resolving to `tests/data/`.
* Use `tempfile::tempdir()` for ephemeral outputs; fixture CSVs stay under 50 KB.
* Property tests use `proptest` (example in `lib.rs::tests`).
* Benchmarks use `criterion` in `benches/` (run with `cargo bench`).
* Always test both success and at least one failure path per public function.
* Add `// Invariant:` comments above test data explaining assumptions.
* Mark slow tests with `#[ignore]`.

## Adding a New Subcommand

1. Define a new `*Args` struct in `cli.rs` with `#[derive(Debug, Args)]`.
2. Add a variant to the `Commands` enum with the doc comment serving as CLI help text.
3. Create a module (`src/<name>.rs`) with `pub fn execute(args: &XArgs) -> Result<()>`.
4. Declare the module in `lib.rs` and add the dispatch arm in `run()` using `run_operation()`.
5. Write integration tests in `tests/` using `assert_cmd`.

## Adding a New Column Type

1. Add a variant to `ColumnType` in `schema.rs` and update serde, `FromStr`, `Display`, and the inference order.
2. Add a corresponding variant to `Value` in `data.rs` with a parsing function.
3. Update `parse_typed_value()` in `data.rs` and `ComparableValue` ordering.
4. Add test fixtures with valid and invalid samples.

## Performance Expectations

* All row processing uses streaming iterators; `collect::<Vec<_>>()` on large datasets requires a justifying comment.
* Index-accelerated reads use seek-based I/O without buffering the full file.
* In-memory sort is the fallback only when no matching index variant exists.
* Benchmarks live in `benches/<area>_<operation>.rs` and use `criterion`.

## Quality Gates

Run these commands before committing (each as a separate invocation):

1. `cargo fmt --check`
2. `cargo clippy -- -D warnings`
3. `cargo test`

## Terminal Command Execution Policy

Run each terminal command as a separate, standalone invocation. Never chain commands with `;`, `&&`, `||`, or `|` except for output redirection.

### Rules

1. One command per terminal call.
2. No `cmd /c` wrappers. Run commands directly in the shell.
3. No exit-code echo suffixes.
4. Inspect output and exit code before running the next command.
5. Always use `pwsh`, never `powershell` or `powershell.exe`.

### Allowed Exceptions

Output redirection is permitted because it is I/O plumbing, not command chaining:

* Shell redirection operators: `>`, `>>`, `2>&1`
* Pipe to `Out-File`, `Set-Content`, or `Out-String`

### Auto-Approve Patterns

```json
{
    ".specify/scripts/bash/": true,
    ".specify/scripts/powershell/": true,
    "/^cargo (build|test|run|clippy|fmt|check|doc|update|install|search|publish|login|logout|new|init|add|upgrade|version|help|bench)(\\s[^;|&`]*)?(\\s*(>|>>|2>&1|\\|\\s*(Out-File|Set-Content|Out-String))\\s*[^;|&`]*)*$/": {
        "approve": true,
        "matchCommandLine": true
    },
    "/^cargo --(help|version|verbose|quiet|release|features)(\\s[^;|&`]*)?$/": {
        "approve": true,
        "matchCommandLine": true
    },
    "/^git (status|add|commit|diff|log|fetch|pull|push|checkout|branch|--version)(\\s[^;|&`]*)?(\\s*(>|>>|2>&1|\\|\\s*(Out-File|Set-Content|Out-String))\\s*[^;|&`]*)*$/": {
        "approve": true,
        "matchCommandLine": true
    },
    "/^(Out-File|Set-Content|Add-Content|Get-Content|Get-ChildItem|Copy-Item|Move-Item|New-Item|Test-Path)(\\s[^;|&`]*)?$/": {
        "approve": true,
        "matchCommandLine": true
    },
    "/^(echo|dir|mkdir|where\\.exe|vsWhere\\.exe|rustup|rustc|refreshenv)(\\s[^;|&`]*)?$/": {
        "approve": true,
        "matchCommandLine": true
    },
    "/^cmd /c \"cargo (test|check|clippy|fmt|build|doc|bench)(\\s[^;|&`]*)?\"(\\s*[;&|]+\\s*echo\\s.*)?$/": {
        "approve": true,
        "matchCommandLine": true
    }
}
```
