# Implementation Plan: CSV-Managed — Baseline SDD Specification

**Branch**: `001-baseline-sdd-spec` | **Date**: 2026-02-13 | **Spec**: [spec.md](spec.md)
**Input**: Feature specification from `/specs/001-baseline-sdd-spec/spec.md`

**Note**: This template is filled in by the `/speckit.plan` command.
See `.specify/templates/commands/plan.md` for the execution workflow.

## Summary

Document the existing csv-managed v1.0.2 baseline as a formal SDD specification.
The tool is a high-performance Rust CLI for streaming, transforming, validating,
indexing, and profiling large CSV/TSV datasets. This plan captures the existing
architecture, data model, and API contracts so future feature work follows
spec-driven development practices with constitution compliance.

## Technical Context

**Language/Version**: Rust 2024 edition, stable toolchain, package v1.0.2
**Primary Dependencies**: clap 4.5 (CLI), csv 1.4 (parsing), serde/serde_yaml 0.9
(schema YAML), chrono 0.4 (temporal), rust_decimal 1 (precision), evalexpr 12
(expressions), bincode 2 (index serialization), encoding_rs 0.8 (transcoding),
sha2 0.10 (snapshots), similar 2 (diff), thiserror 2 (errors), uuid 1 (GUID)
**Storage**: File-based — CSV/TSV input/output, YAML schemas, binary `.idx` index
files, JSON snapshots. No database dependency.
**Testing**: `cargo test` with `assert_cmd`/`predicates` (CLI integration),
`proptest` (property), `criterion` (benchmarks), `tempfile` (fixtures)
**Target Platform**: Windows x86_64, Linux x86_64/musl, macOS aarch64/x86_64
**Project Type**: Single Rust binary crate with library (`main.rs` + `lib.rs`)
**Performance Goals**: Stream hundreds-of-GB files with bounded memory; index-
accelerated sort proportional to I/O not row count; sub-second schema inference
on 2000-row samples
**Constraints**: Minimal memory footprint via streaming iterators; no full-file
buffering except explicit in-memory sort fallback; deterministic output ordering
**Scale/Scope**: 20 source modules, ~9,500 LOC in `src/`, ~4,100 LOC in `tests/`,
59 functional requirements, 10 user stories, 6 CLI subcommands

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Status | Evidence |
|-----------|--------|----------|
| I. Streaming / Iterators | PASS | All commands use `csv::Reader` with `records()`/`byte_records()` streaming; index sort uses seek-based reads |
| II. Separation of Concerns | PASS | 20 distinct modules: `cli`, `schema`, `data`, `index`, `process`, `filter`, `expr`, `derive`, `stats`, `frequency`, `verify`, `append`, `io_utils`, `table`, `columns`, `rows`, `join`, `schema_cmd`, `install`, `lib` |
| III. Zero-Copy / Borrowing | PASS | Functions accept `&str`/`&[u8]` where possible; `Cow` used for conditional ownership in encoding paths |
| IV. Explicit Error Types | PASS | Uses `anyhow::Result` at boundaries, `thiserror` for custom error enums, `?` propagation throughout |
| V. Deterministic Performance | PASS | No hidden global state; costly features gated behind flags (`--apply-mappings`, `--frequency`, `--covering`) |
| VI. Extensibility via Traits | PASS | `ColumnType` enum with trait-like dispatch for parsing/display; `Value` enum with `Ord`/`Eq` for generic comparison |
| VII. Config-First | PASS | YAML schema files as canonical input; JSON pipeline definitions supported |
| Rust Coding Standards | PASS | `rustfmt` enforced, `clippy -D warnings` clean, Rustdoc on public items |
| Testing Strategy | PASS | Unit tests inline, integration in `tests/`, property tests with `proptest`, benchmarks with `criterion` |

**Gate result**: ALL PASS — proceed to Phase 0.

## Project Structure

### Documentation (this feature)

```text
specs/001-baseline-sdd-spec/
├── plan.md              # This file (/speckit.plan command output)
├── research.md          # Phase 0 output (/speckit.plan command)
├── data-model.md        # Phase 1 output (/speckit.plan command)
├── quickstart.md        # Phase 1 output (/speckit.plan command)
├── contracts/           # Phase 1 output (/speckit.plan command)
│   └── cli-contract.md  # CLI command interface contracts
└── tasks.md             # Phase 2 output (/speckit.tasks command)
```

### Source Code (repository root)

```text
src/
├── main.rs              # Entry point — delegates to lib::run()
├── lib.rs               # Crate root — module declarations, CLI dispatch, timing
├── cli.rs               # CLI argument definitions (clap derive)
├── schema.rs            # Schema model, inference, YAML I/O, type system (3167 LOC)
├── schema_cmd.rs         # Schema subcommands: probe, infer, verify, columns (974 LOC)
├── data.rs              # Value enum, typed parsing, currency/decimal (865 LOC)
├── index.rs             # B-Tree index build/save/load, variant selection (818 LOC)
├── process.rs           # Process command: sort, filter, project, derive (753 LOC)
├── stats.rs             # Summary statistics computation (589 LOC)
├── frequency.rs         # Frequency/top-N analysis (261 LOC)
├── expr.rs              # Expression engine wrapping evalexpr (360 LOC)
├── filter.rs            # Row-level filter parsing and evaluation (175 LOC)
├── derive.rs            # Derived column specification and evaluation (63 LOC)
├── verify.rs            # Schema verification engine (314 LOC)
├── append.rs            # Multi-file append with header validation (168 LOC)
├── io_utils.rs          # I/O helpers: encoding, delimiter, reader/writer (238 LOC)
├── table.rs             # ASCII table renderer (141 LOC)
├── rows.rs              # Row parsing and filter evaluation helpers (40 LOC)
├── columns.rs           # Schema columns display (43 LOC)
├── join.rs              # Join engine (dormant, 361 LOC)
└── install.rs           # Self-install via cargo (46 LOC)

tests/
├── cli.rs               # End-to-end CLI integration tests (898 LOC)
├── preview.rs           # Preview mode tests (285 LOC)
├── probe.rs             # Schema probe tests (100 LOC)
├── process.rs           # Process command tests (1226 LOC)
├── schema.rs            # Schema subcommand tests (846 LOC)
├── stats.rs             # Stats command tests (499 LOC)
├── stdin_pipeline.rs    # Stdin pipeline tests (255 LOC)
└── data/                # Test fixtures (CSV files and schema YAMLs)

benches/
└── index_vs_sort.rs     # Index vs in-memory sort benchmark

docs/                    # Reference documentation (17 files)
```

**Structure Decision**: Single Rust binary crate (Option 1) with flat module
layout under `src/`. Integration tests in `tests/` following the Rust convention.
This matches the existing repository structure — no reorganization needed for
the baseline spec.

## Complexity Tracking

> No constitution violations detected. No justifications required.
