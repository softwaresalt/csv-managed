# Quickstart: CSV-Managed — Baseline SDD Specification

**Branch**: `001-baseline-sdd-spec` | **Date**: 2026-02-13

## Prerequisites

- Rust stable toolchain (edition 2024)
- Cargo (included with Rust)

## Build

```bash
cargo build --release
```

The binary is produced at `target/release/csv-managed` (or `.exe` on Windows).

## Verify the Build

```bash
cargo test --all
cargo clippy --all-targets --all-features -- -D warnings
```

## Core Workflows

### 1. Discover a CSV's structure

```bash
# Quick probe — displays inferred types without writing anything
csv-managed schema probe -i data.csv

# Full probe with mapping suggestions
csv-managed schema probe -i data.csv --mapping --sample-rows 5000
```

### 2. Create a reusable schema

```bash
# Infer and persist
csv-managed schema infer -i data.csv -o data-schema.yml

# Preview before writing
csv-managed schema infer -i data.csv --preview

# Compare with an existing schema
csv-managed schema infer -i data.csv --diff existing-schema.yml
```

### 3. Verify data against a schema

```bash
# Summary report
csv-managed schema verify -m data-schema.yml -i data.csv

# Detailed report (first 50 violations)
csv-managed schema verify -m data-schema.yml -i data.csv --report-invalid detail 50

# Verify multiple files
csv-managed schema verify -m data-schema.yml -i jan.csv -i feb.csv -i mar.csv
```

### 4. Transform data

```bash
# Filter + sort + project
csv-managed process -i data.csv -m data-schema.yml \
  --filter "amount >= 100" \
  --sort order_date:asc \
  --columns name,email,amount \
  -o filtered.csv

# Derive computed columns
csv-managed process -i orders.csv \
  --derive "total_tax=amount*0.0825" \
  --derive "ship_lag=date_diff_days(shipped_at,ordered_at)" \
  -o enriched.csv

# Preview results as a table
csv-managed process -i data.csv --preview --limit 20

# Boolean normalization
csv-managed process -i data.csv --boolean-format true-false -o normalized.csv
```

### 5. Build indexes for large-file sorting

```bash
# Single variant index
csv-managed index -i data.csv -o data.idx --spec default=order_date:asc,customer_id:asc

# Covering index (generates all direction permutations)
csv-managed index -i data.csv -o data.idx --covering geo=date:asc|desc,customer:asc

# Use an index for fast sorting
csv-managed process -i data.csv -x data.idx --sort order_date:asc -o sorted.csv
```

### 6. Compute statistics

```bash
# Summary statistics for numeric columns
csv-managed stats -i data.csv

# Frequency counts, top 10
csv-managed stats -i data.csv --frequency --top 10

# Filtered statistics
csv-managed stats -i data.csv -m data-schema.yml --filter "region = US"
```

### 7. Append multiple files

```bash
csv-managed append -i jan.csv -i feb.csv -i mar.csv -o q1.csv

# With schema validation during append
csv-managed append -i jan.csv -i feb.csv -m data-schema.yml -o q1.csv
```

### 8. Pipeline composition

```bash
# Pipe process output into stats
csv-managed process -i data.csv --filter "status = shipped" -o - | \
  csv-managed stats -i -

# Encoding transcoding in a pipeline
csv-managed process -i legacy.csv --input-encoding windows-1252 --output-encoding utf-8 -o modern.csv
```

## Key Files

| File | Purpose |
|------|---------|
| `Cargo.toml` | Package manifest and dependencies |
| `src/lib.rs` | Crate root, command dispatch |
| `src/cli.rs` | CLI argument definitions |
| `src/schema.rs` | Schema model, inference, YAML I/O |
| `src/data.rs` | Value types, parsing |
| `src/index.rs` | B-Tree index build/load |
| `src/process.rs` | Process command execution |
| `docs/` | Reference documentation |
| `tests/` | Integration test suite |

## Next Steps

- Read [spec.md](spec.md) for the full feature specification
- Read [data-model.md](data-model.md) for entity definitions
- Read [contracts/cli-contract.md](contracts/cli-contract.md) for complete CLI contracts
- Run `csv-managed --help` for built-in command help
