<!-- markdownlint-disable-file -->
# PR Review Status: 001-baseline-sdd-spec

## Review Status

* Phase: 2 (Analyze Changes — complete)
* Last Updated: 2026-02-14
* Summary: Full baseline codebase review of 21 source files (~9,400 LOC) across 13 phases of feature implementation

## Branch and Metadata

* Normalized Branch: `001-baseline-sdd-spec`
* Source Branch: `001-baseline-sdd-spec`
* Base Branch: `main`
* Linked Work Items: Spec 001-baseline-sdd-spec (13 phases, 144 tasks, 59 FRs)

## Diff Mapping

| File | Type | New Lines | Notes |
|------|------|-----------|-------|
| src/lib.rs | New | 1–245 | Crate root, CLI dispatch, run_operation() |
| src/main.rs | New | 1–11 | Entry point |
| src/cli.rs | New | 1–404 | clap derive definitions |
| src/schema.rs | New | 1–3195 | Schema model, inference, mapping, serde |
| src/schema_cmd.rs | New | 1–987 | Schema subcommand dispatch |
| src/data.rs | New | 1–963 | Value enum, typed parsers |
| src/process.rs | New | 1–783 | Process subcommand |
| src/filter.rs | New | 1–245 | Filter parsing and evaluation |
| src/expr.rs | New | 1–563 | Expression engine |
| src/index.rs | New | 1–933 | B-tree index |
| src/io_utils.rs | New | 1–251 | I/O utilities |
| src/verify.rs | New | 1–326 | Schema verification |
| src/append.rs | New | 1–200 | Multi-file append |
| src/stats.rs | New | 1–603 | Summary statistics |
| src/frequency.rs | New | 1–275 | Frequency analysis |
| src/derive.rs | New | 1–75 | Derived columns |
| src/rows.rs | New | 1–50 | Row helpers |
| src/columns.rs | New | 1–40 | Column listing |
| src/table.rs | New | 1–160 | ASCII table rendering |
| src/install.rs | New | 1–46 | Self-install |
| src/join.rs | New | 1–361 | Join (commented out in dispatch) |

## Instruction Files Reviewed

* `.github/instructions/rust.instructions.md`: Applies to all `**/*.rs` — Rust conventions, no unwrap, no unsafe, Rustdoc
* `.github/copilot-instructions.md`: Architecture rules — streaming, anyhow, no println from deep logic
* `specs/001-baseline-sdd-spec/plan.md`: Constitution — no unsafe, no unwrap/expect in lib code

## Review Items

### 🔍 In Review

_Items queued for Phase 3 collaborative review below._

#### RI-01: Value::Ord panics on heterogeneous variants

* File: `src/data.rs`
* Lines: 274
* Category: Correctness
* Severity: CRITICAL

**Description**: The `Ord` implementation for `Value` panics with `"Cannot compare heterogeneous Value variants"`. This is reachable from `compare_rows()` in `process.rs` during sort operations. If a nullable column produces `Value::Null` alongside typed values, or if schema inference assigns wrong types, the process aborts with a panic instead of producing a meaningful error.

**Suggested Resolution**: Replace `panic!` with deterministic ordering using `std::mem::discriminant` comparison as a fallback.

---

#### RI-02: Sole `unsafe` block in codebase

* File: `src/io_utils.rs`
* Lines: 197
* Category: Security / Convention
* Severity: HIGH

**Description**: `unsafe { std::str::from_utf8_unchecked(valid_slice) }` — while logically safe because `valid_up_to` from `Utf8Error` guarantees the slice is valid UTF-8, this is the only `unsafe` block in the entire codebase and violates the constitution's "no unsafe" rule. The safe alternative has negligible overhead.

**Suggested Resolution**: Replace with `std::str::from_utf8(valid_slice).expect("valid_up_to guarantees valid UTF-8")` or better yet `std::str::from_utf8(valid_slice).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?`.

---

#### RI-03: `.expect()` on user-triggered paths in schema_cmd.rs

* File: `src/schema_cmd.rs`
* Lines: 231, 276, 431
* Category: unwrap/expect violation
* Severity: HIGH

**Description**: Three `.expect()` calls on paths reachable from user CLI input:
  - L231: `.expect("Preview requires serialized YAML output")` — if yaml_output is None when preview is requested
  - L276: `.expect("Diff requires serialized YAML output")` — if yaml_output is None for diff
  - L431: `.expect("column should exist")` — after column lookup (logically guarded but convention violation)

**Suggested Resolution**: Replace all three with `.context(...)` + `?` propagation.

---

#### RI-04: `unwrap()` in stats.rs median sort

* File: `src/stats.rs`
* Lines: 349
* Category: unwrap violation
* Severity: MEDIUM

**Description**: `a.partial_cmp(b).unwrap()` inside the `median()` sort. If any `f64::NAN` value reaches this code path, it panics.

**Suggested Resolution**: Use `a.total_cmp(b)` (stable since Rust 1.62) which handles NaN deterministically.

---

#### RI-05: `.expect()` calls in frequency.rs

* File: `src/frequency.rs`
* Lines: 155, 159
* Category: unwrap/expect violation
* Severity: MEDIUM

**Description**: Two `.expect()` calls on HashMap lookups that are logically safe but violate the no-expect convention.

**Suggested Resolution**: Replace with `.context(...)` + `?`.

---

#### RI-06: `.expect()` calls in schema.rs

* File: `src/schema.rs`
* Lines: 718, 2281, 2298, 2372
* Category: unwrap/expect violation
* Severity: MEDIUM

**Description**: Four `.expect()` calls on paths that are logically guarded but violate convention:
  - L718: `DecimalSpec::new()` — guaranteed by FixedDecimalValue
  - L2281: `.first().expect(...)` — guarded by `has_mappings()`
  - L2298: `.last().expect(...)` — same guard
  - L2372: `previous_to.expect(...)` — guarded by non-empty loop iteration

**Suggested Resolution**: Replace with `.context(...)` + `?` or `.ok_or_else(|| anyhow!(...))` + `?`.

---

#### RI-07: Missing module-level Rustdoc

* File: `src/columns.rs`, `src/install.rs`, `src/join.rs`
* Lines: 1 (all three files)
* Category: Convention
* Severity: LOW

**Description**: Three source files lack `//!` module-level documentation.

**Suggested Resolution**: Add `//!` doc comments at the top of each file.

---

### ✅ Approved for PR Comment

_None yet — pending Phase 3 decisions._

### ❌ Rejected / No Action

#### Deferred: schema.rs size (3,195 lines)

Architectural refactoring to split into submodules. Out of scope for this PR — tracked as future work.

#### Deferred: process.rs execute() length (210 lines)

Function exceeds 100-line guideline. Refactoring risk too high for this PR.

#### Deferred: Unbounded Vec in stats median (P1)

Requires streaming median algorithm. Documented in Phase 13 memory as future optimization.

#### Deferred: println! in verify.rs/schema_cmd.rs

These are at or near the CLI output boundary. Debatable whether they violate the convention.

#### Acceptable: eprintln in schema.rs L209

Guarded by `#[cfg(test)]` AND env var check — only present in test builds. Not a production concern.

## Next Steps

* [ ] Phase 3: Present RI-01 through RI-07 to user for decisions
* [ ] Fix approved items
* [ ] Run quality gates (cargo test, clippy, doc)
* [ ] Phase 4: Create PR to main
