# csv-managed

`csv-managed` is a high‑performance Rust CLI for exploring, validating, transforming, and indexing very large CSV/TSV (and future delimited) datasets using streaming, typed schemas, and multi‑variant indexes.

## Feature Matrix (Concise)

| Area | Highlights |
|------|-----------|
| Delimiters & Encodings | Comma/tab/pipe/semicolon/custom; independent input/output encoding; stdin/stdout streaming |
| Schema Discovery | Sample or full scan inference; diff, overrides, placeholder normalization, snapshots |
| Header Detection | Automatic header/headerless with synthetic `field_#`; force via `--assume-header` |
| Datatype Transformations | Ordered `datatype_mappings` chains (parse, round, trim, case) before final typing |
| Decimal & Currency | Fixed `decimal(p,s)` (≤28 precision) and currency scale (2 or 4) enforcement |
| Indexing & Sorting | Multi-variant B-Tree index; longest matching prefix acceleration; covering expansion |
| Filtering & Derivation | Typed comparisons + Evalexpr expressions; temporal helpers; positional aliases |
| Verification | Streaming per-cell type enforcement; tiered invalid reporting |
| Statistics & Frequency | Numeric + temporal metrics; distinct counts with `--frequency` / `--top` |
| Append & Pipelines | Multi-file union with schema consistency; efficient chained stdin workflows |
| Boolean & Table Output | Configurable boolean formats; elastic preview/table rendering |
| Snapshots | Layout/inference regression guard (`--snapshot`) |
| Error & Logging | Contextual failures; debug logging for inference/index/mappings |

Extended details moved to dedicated docs (see Documentation Map).

---

## Global Documentation Table of Contents

### Core (This README)

1. [Feature Matrix](#feature-matrix-concise)
2. [Global Documentation TOC](#global-documentation-table-of-contents)
3. [Quick Start](#quick-start)
4. [Installation](#installation)
5. [Core Concepts (Brief)](#core-concepts-brief)
6. [Datatypes](#datatypes-supported)
7. [Expressions Overview](#expressions--derived-logic-overview)
8. [Indexes & Sorting Overview](#indexes--sorting-overview)
9. [Streaming & Pipelines Overview](#streaming--pipelines-overview)
10. [Command Guide](#command-guide-summary)
11. [Advanced Topics](#advanced-topics)
12. [Roadmap](#roadmap)
13. [Contributing](#contributing)
14. [License](#license)
15. [Support](#support)

### Deep Dives (Docs Directory)

1. [Schema Inference Internals](docs/schema-inference.md)
2. [Schema Command Examples](docs/schema-examples.md)
3. [Datatype Mappings Deep Dive](docs/datatype-mappings.md)
4. [Statistics & Frequency Deep Dive](docs/stats.md)
5. [Header Detection & FAQ](docs/header-detection.md)
6. [Naming Conventions](docs/naming-conventions.md)
7. [Snapshots vs Verification](docs/snapshots-and-verification.md)
8. [Expressions Reference & Extended Examples](docs/expressions.md)
9. [Indexing & Sorting Guide](docs/indexing-and-sorting.md)
10. [Pipelines & Multi-Stage Patterns](docs/pipelines.md)
11. [Encoding Normalization](docs/encoding-normalization.md)
12. [Boolean Formatting & Table Output](docs/boolean-formatting.md)
13. [Operational Notes (Perf / Errors / Logging / Testing)](docs/operations.md)
14. [CLI Help Reference](docs/cli-help.md)

### Quick Cross-Reference

| Capability | Primary Doc |
|------------|-------------|
| Inference algorithm details | [schema-inference](docs/schema-inference.md) |
| Placeholder / NA handling | [schema-inference](docs/schema-inference.md), [schema-examples](docs/schema-examples.md) |
| Decimal & Currency rules | [schema-inference](docs/schema-inference.md), [datatype-mappings](docs/datatype-mappings.md) |
| Mapping strategies & strategies matrix | [datatype-mappings](docs/datatype-mappings.md) |
| Overrides vs mappings vs replacements | [schema-examples](docs/schema-examples.md), [datatype-mappings](docs/datatype-mappings.md) |
| Header detection heuristic | [header-detection](docs/header-detection.md) |
| Naming / snake_case rationale | [naming-conventions](docs/naming-conventions.md) |
| Snapshot vs verify comparison | [snapshots-and-verification](docs/snapshots-and-verification.md) |
| Invalid reporting tiers | [snapshots-and-verification](docs/snapshots-and-verification.md) |
| Index variant design & covering | [indexing-and-sorting](docs/indexing-and-sorting.md) |
| Streaming pipeline safety (header shape) | [pipelines](docs/pipelines.md) |
| Encoding normalization patterns | [encoding-normalization](docs/encoding-normalization.md) |
| Boolean output modes | [boolean-formatting](docs/boolean-formatting.md) |
| Statistics aggregation & frequency counting | [stats](docs/stats.md) |
| Expressions functions, quoting, bucketing | [expressions](docs/expressions.md) |
| Performance & logging guidance | [operations](docs/operations.md) |
| CLI option reference | [cli-help](docs/cli-help.md) |

> Use this TOC as a hub: internal anchors for quick orientation; deep dives for authoritative detail.

---

## Documentation Map

| Topic | Doc |
|-------|-----|
| Expressions (full reference & examples) | [expressions](docs/expressions.md) |
| Indexing & Sorting internals | [indexing-and-sorting](docs/indexing-and-sorting.md) |
| Multi-stage pipelines & header shape rules | [pipelines](docs/pipelines.md) |
| Schema inference internals | [schema-inference](docs/schema-inference.md) |
| Schema command usage examples | [schema-examples](docs/schema-examples.md) |
| Header detection algorithm & FAQ | [header-detection](docs/header-detection.md) |
| Naming conventions (snake_case rationale) | [naming-conventions](docs/naming-conventions.md) |
| Snapshots vs verification + reporting tiers | [snapshots-and-verification](docs/snapshots-and-verification.md) |
| Boolean formatting & table output | [boolean-formatting](docs/boolean-formatting.md) |
| Encoding normalization pipelines | [encoding-normalization](docs/encoding-normalization.md) |
| Datatype mappings & transformation strategies | [datatype-mappings](docs/datatype-mappings.md) |
| Statistics & frequency metrics | [stats](docs/stats.md) |
| Operational notes (performance, errors, logging, testing) | [operations](docs/operations.md) |
| CLI flag reference (captured help output) | [cli-help](docs/cli-help.md) |

Roadmap/backlog: see the [roadmap](.plan/backlog.md) (if present).

---

## Quick Start

```powershell
# 1. Infer schema
./target/release/csv-managed.exe schema infer -i ./data/orders.csv -o ./data/orders-schema.yml --sample-rows 0
# 2. Build indexes
./target/release/csv-managed.exe index -i ./data/orders.csv -o ./data/orders.idx --spec default=order_date:asc,customer_id:asc --spec recent=order_date:desc -m ./data/orders-schema.yml
# 3. Typed processing (filters / derives / sort)
./target/release/csv-managed.exe process -i ./data/orders.csv -m ./data/orders-schema.yml -x ./data/orders.idx --index-variant default --sort order_date:asc,customer_id:asc --filter "status = shipped" --derive 'total_with_tax=amount*1.0825' --row-numbers -o ./data/orders_filtered.csv
# 4. Stats (numeric & temporal)
./target/release/csv-managed.exe stats -i ./data/orders.csv -m ./data/orders-schema.yml
# 5. Frequency counts
./target/release/csv-managed.exe stats -i ./data/orders.csv -m ./data/orders-schema.yml --frequency --top 10
# 6. Preview (no file output allowed with --preview)
./target/release/csv-managed.exe process -i ./data/orders.csv --preview --limit 15
```

> See extended examples in collapsible sections throughout this README.

---

## Installation

```bash
cargo build --release
```

Binary (Windows): `target\release\csv-managed.exe`

From crates.io:

```bash
cargo install csv-managed
```

Local path dev install:

```bash
cargo install --path .
```

Helper command (wraps `cargo install`):

```powershell
./target/release/csv-managed.exe install --locked
```

Environment logging examples:

```powershell
$env:RUST_LOG='info'
```

```batch
set RUST_LOG=info
```

---

## Core Concepts (Brief)

Schemas declare column order, types, optional renames, mapping chains, and replacements. Per-cell flow: raw → mappings → replacements → final parse. See `docs/schema-inference.md` and `docs/schema-examples.md`.

Header detection, naming guidance, and FAQ: `docs/header-detection.md`, `docs/naming-conventions.md`.

Overrides vs mappings vs replacements decision table: `docs/schema-examples.md`.

Verification tiers + snapshot comparison: `docs/snapshots-and-verification.md`.

### Snapshot Internals (Deep Dive)

Snapshot includes: header+type hash (SHA-256), textual inference table, observation summaries. Hash changes on any header reorder or type change. Regenerate intentionally after approved inference adjustments.

---

## Datatypes (Supported)

| Type | Examples | Notes |
|------|----------|-------|
| String | any UTF‑8 | Post-mapping names usable in expressions |
| Integer | `42`, `-7` | 64-bit signed |
| Float | `3.14`, `2` | f64 (integers accepted) |
| Boolean | `true/false`, `yes/no`, `1/0` | Input variants normalized; output format selectable |
| Date | `2024-08-01`, `08/01/2024` | Canonical `YYYY-MM-DD` |
| DateTime | `2024-08-01T13:45:00` | Naive (no TZ) |
| Time | `06:00:00`, `14:30` | Canonical `HH:MM:SS` |
| Currency | `$12.34`, `123.4567` | Enforce 2 or 4 scale; symbol stripped |
| Decimal | `123.4567`, `(1,234.50)` | Fixed precision/scale ≤28 |
| Guid | RFC 4122 hyphenated or 32hex | Case-insensitive |

---

## Expressions & Derived Logic (Overview)

Derived columns: `--derive name=expr`  •  Filters: `--filter`, `--filter-expr`  •  Positional aliases: `c0, c1, ...`  •  `row_number` when `--row-numbers` enabled.

### Quick Cheat Sheets

| Pattern | Example | Description |
|---------|---------|-------------|
| Arithmetic | `total_with_tax=amount*1.0825` | Multiply numeric column |
| Conditional flag | `high_value=if(amount>1000,1,0)` | 1/0 indicator |
| Date diff | `ship_lag=date_diff_days(shipped_at,ordered_at)` | Days between dates |
| Time diff | `window=time_diff_seconds(end_time,start_time)` | Seconds difference |
| Concat | `channel_tag=concat(channel,"-",region)` | Combine strings |
| Guid passthrough | `id_copy=id` | Duplicate column |
| Row number | `row_index=row_number` | Sequential index |

| Aspect | `--filter` | `--filter-expr` |
|--------|-----------|-----------------|
| Operators | Basic typed comparisons | Full Evalexpr syntax |
| Logic | AND via repetition | AND/OR, nested `if` |
| Temporal helpers | Direct typed compare | `date_diff_days`, etc. |
| Complexity | Concise | Arbitrary expression |

### Full Expression Reference

**Temporal Helpers**: `date_add`, `date_sub`, `date_diff_days`, `date_format`, `datetime_add_seconds`, `datetime_diff_seconds`, `datetime_format`, `datetime_to_date`, `datetime_to_time`, `time_add_seconds`, `time_diff_seconds`.

**Pitfalls**:

* PowerShell quoting: wrap whole expression in single quotes, internal literals in double quotes.
* `c0` is first column (0-based). Verify mapping order.
* `row_number` exists only if `--row-numbers` set.
* Use helpers, not raw string comparisons, for temporal correctness.
* Mapping chains precede replacements which precede final parse; expressions see normalized values.

**Function Index (alphabetical)**: `concat`, `date_add`, `date_diff_days`, `date_format`, `date_sub`, `datetime_add_seconds`, `datetime_diff_seconds`, `datetime_format`, `datetime_to_date`, `datetime_to_time`, `if`, `time_add_seconds`, `time_diff_seconds`.

**Debugging**: Increase logging with `RUST_LOG=csv_managed=debug`. Future deep expression tracing may emit `expr:` prefixed debug lines.

---

## Indexes & Sorting (Overview)

Indexes store byte offsets keyed by concatenated column values. A single `.idx` contains multiple named variants (different column sequences and directions). `process` chooses the variant with the longest matching prefix for a requested `--sort` unless `--index-variant` pins a specific one.

**Building**:

```powershell
./target/release/csv-managed.exe index -i ./data/orders.csv -o ./data/orders.idx \
  --spec default=order_date:asc,customer_id:asc \
  --spec recent=order_date:desc -m ./data/orders-schema.yml
```

**Covering** (`--covering`): Generate systematic direction/prefix permutations from a concise pattern (e.g. `geo=date:asc|desc,customer:asc`).

Fallback: When no index variant matches the entire sort signature, an in-memory stable multi-column sort executes (still streaming transforms earlier/later as possible).

---

## Streaming & Pipelines (Overview)

Use `-i -` to read from stdin; schema strongly recommended for typed semantics. Each stage must explicitly declare stdin usage. Avoid header shape changes between typed stages unless you also provide a matching updated schema.

Core guidelines:

* Keep early projections narrow.
* Apply filters before sorting or heavy derives.
* Normalize encodings up front (`--input-encoding` / `--output-encoding`).
* Use `--preview --limit` for fast inspection; remove before chaining downstream.

### Extended Pipeline Examples & Troubleshooting

**Filter then stats**:

```powershell
Get-Content .\tests\data\big_5_players_stats_2023_2024.csv | \
  .\target\release\csv-managed.exe process -i - --schema .\tests\data\big_5_players_stats-schema.yml \
  --filter "Performance_Gls >= 10" --limit 40 | \
  .\target\release\csv-managed.exe stats -i - --schema .\tests\data\big_5_players_stats-schema.yml -C Performance_Gls
```

**Append with one streamed input**:

```powershell
Get-Content .\tests\data\big_5_players_stats_2023_2024.csv | \
  .\target\release\csv-managed.exe append -i - -i .\tmp\big_5_preview.csv \
  --schema .\tests\data\big_5_players_stats-schema.yml -o .\tmp\players_union.csv
```

**Encoding normalization**:

```powershell
Get-Content .\tmp\big_5_windows1252.csv | \
  .\target\release\csv-managed.exe process -i - --input-encoding windows-1252 \
  --schema .\tests\data\big_5_players_stats-schema.yml --columns Player --columns Squad --limit 5 --table
```

**Troubleshooting**:

| Symptom | Cause | Fix |
|---------|-------|-----|
| Hang | Upstream not producing | Add `--preview --limit` to inspect |
| Column not found | Rename/mapping changed | Re-check `schema columns` / header output |
| Zero stats rows | Filters excluded all rows | Relax/remove filters |
| Invalid datatype downstream | Schema mismatch | Supply correct schema per stage |

---

## Command Guide (Summary)

Concise flag references; see concept sections for deep behavior.

### schema

Probe, infer, verify, list columns, diff and snapshot inference output.

| Sub/Flag | Summary |
|----------|---------|
| `probe` | Inference preview table (no file) |
| `infer` | Inference + optional write (`-o`) + diff/snapshot integration |
| `verify` | Streaming type & replacement validation |
| `columns` | Tabular listing of schema columns |
| `--snapshot` | Layout regression guard |
| `--diff <schema>` | Unified diff vs existing schema |
| `--assume-header` | Override header detection |
| `--mapping` | Emit mapping scaffold & snake_case suggestions |
| `--replace-template` | Inject empty `replace` arrays |

### process

Transform & emit rows: filtering, derives, column selection, sorting (indexed or fallback), boolean formatting, row numbering, preview/table output.

### stats

Numeric & temporal summary metrics; `--frequency` for distinct counts; filter integration.

### append

Concatenate multiple CSV inputs enforcing header/schema consistency.

### index

Build multi-variant B-tree index files (`--spec`, `--covering`) for accelerated sort alignment.

### install

Wrapper around `cargo install csv-managed` (version / force / locked / root flags).

### schema columns

List schema-declared columns and datatypes (resolves renames).

---

## Advanced Topics

### Performance Considerations

* Indexed sort avoids retaining all rows in memory.
* Early filtering diminishes downstream CPU & sort footprint.
* Median requires buffering column values; limit wide median usage on huge datasets.
* Decimal & currency parsing add overhead—declare only where needed.

### Error Handling

`anyhow` contexts annotate origin (I/O, parse, schema, expression). Fast failure on unknown columns, invalid expressions, header mismatches, precision overflow, unsupported mapping strategies.

### Logging

Set `RUST_LOG=csv_managed=debug` for phase insights (inference voting, index selection, mapping application). Higher verbosity may impact throughput—toggle only when diagnosing.

### Testing

Run `cargo test`. Integration tests cover inference, indexing, process flags, piping, stats. Use `assert_cmd` for pipeline locking. Add new tests for any behavior that changes output formatting (update snapshots intentionally).

---

## Roadmap

See consolidated backlog & release planning in `[.plan/backlog.md](.plan/backlog.md)` for upcoming features (join redesign, primary key indexes, batch definition ingestion, additional file formats).

---

## Contributing

1. Fork & branch (`feat/<name>`).  
2. Add unit + integration tests.  
3. `cargo fmt && cargo clippy && cargo test` must pass.  
4. Update README sections or move features from roadmap to implemented list.  

## License

See `LICENSE`.

## Support

Open issues for bugs, enhancements, or documentation gaps. Pull requests welcome.

---

## Documentation Notes

Deep dive sections removed from README and relocated to `docs/`. Use the Documentation Map above for full references.

---
