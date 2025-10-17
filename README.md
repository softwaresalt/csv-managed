# csv_managed

`csv_managed` is a high-performance command-line utility, written in Rust, for exploring and transforming CSV datasets of any size. It offers schema inference, typed filtering, derived columns, on-disk indexing, and configurable input/output formats.

## Getting Started

```bash
# build in release mode
cargo build --release

# view the CLI help
cargo run -- --help
```

Set `RUST_LOG=info` (or `debug`) to enable structured logging.

## Core Commands

### Probe

Infer column data types and save them in a `.meta` JSON file:

```bash
csv_managed probe \
  --input ./data/orders.csv \
  --meta ./data/orders.meta \
  --delimiter ';' \
  --sample-rows 10000
```

### Index

Generate a B-Tree index over one or more columns to accelerate subsequent processing:

```bash
csv_managed index \
  --input ./data/orders.csv \
  --index ./data/orders.idx \
  --columns ordered_at,customer_id \
  --meta ./data/orders.meta
```

### Process

Sort, filter, project, and derive new columns while streaming rows to stdout or a file:

```bash
csv_managed process \
  --input ./data/orders.csv \
  --meta ./data/orders.meta \
  --index ./data/orders.idx \
  --sort ordered_at:desc,customer_id:asc \
  --filter "status = shipped" \
  --derive "total_with_tax=amount*1.0825" \
  --columns order_id,customer_id,amount,total_with_tax \
  --row-numbers \
  --output ./data/orders_sorted.csv
```

Use `--delimiter` and `--output-delimiter` to switch between comma, tab, pipe, semicolon, or any ASCII separator.

## Testing

The project ships with integration tests that exercise the primary workflows:

```bash
cargo test
```

## Roadmap & Extensibility

- Additional filter operators and expression helpers
- Parallel execution for CPU-bound operations
- Cached index metadata and statistics
- Pluggable serialization for schema/index formats

Contributions and issues are welcome!
