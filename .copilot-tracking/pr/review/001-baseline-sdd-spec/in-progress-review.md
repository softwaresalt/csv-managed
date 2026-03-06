<!-- markdownlint-disable-file -->
# PR Review Status: 001-baseline-sdd-spec

## Review Status

* Phase: Complete ✅
* Last Updated: 2026-03-06
* Summary: Full baseline codebase review of 21 source files (~10,700 LOC). All 7 prior RI items resolved. NEW-01 fixed (schema.rs:1751 unwrap removed). NEW-02 documented as non-blocking follow-up. Quality gates: build ✅ clippy -D warnings ✅ tests 229/229 ✅ fmt ✅

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

_All prior items resolved. New items below._

#### NEW-01: `.unwrap()` in production code — `schema.rs:1751`

* File: `src/schema.rs`
* Lines: 1751
* Category: Convention
* Severity: LOW

**Description**: `chars.first().copied().unwrap()` in `build_header_aliases()` (private function). The `!sanitized.is_empty()` guard on line 1747 makes this logically infallible, but it still violates the no-unwrap convention. The `Vec<char>` intermediate allocation also adds a small heap cost on every header; this can be replaced with a direct `chars()` call.

**Suggested Resolution**:
```rust
// Replace lines 1749–1754 with:
if sanitized.len() >= 2 {
    let mut ch_iter = sanitized.chars();
    if let (Some(first), Some(last)) = (ch_iter.next(), ch_iter.last().or(ch_iter.next())) {
        try_insert(&format!("{first}{last}"));
    }
}
```
Or more simply:
```rust
if sanitized.len() >= 2 {
    let first = sanitized.chars().next().expect("sanitized is non-empty by guard");
    let last  = sanitized.chars().next_back().unwrap_or(first);
    try_insert(&format!("{first}{last}"));
}
```
The cleanest idiomatic fix removes the `Vec<char>` entirely. Either eliminates the `unwrap()`.

---

#### NEW-02: 216 public items lack `///` Rustdoc comments

* Files: `src/data.rs`, `src/schema.rs`, `src/io_utils.rs`, `src/process.rs`, `src/cli.rs`, `src/join.rs`, and others
* Category: Documentation / Convention
* Severity: LOW

**Description**: Running `RUSTFLAGS="-W missing-docs" cargo build` reports **216 warnings** covering missing doc comments on public functions (36), methods (46+11), structs (27), struct fields (49), enum variants (35), enums (9), and constants (3). The project constitution requires `///` Rustdoc on all public items. While module-level `//!` comments were added (RI-07 fixed), per-item inline docs are absent for most types.

Representative examples:
- `src/data.rs`: `pub fn parse_naive_date`, `pub struct FixedDecimalValue`, all its methods, `pub fn parse_typed_value`, `pub fn value_to_evalexpr`, etc.
- `src/io_utils.rs`: all 10+ public functions
- `src/schema.rs`: `pub struct CsvLayout`, `pub enum PlaceholderPolicy`, `pub struct DecimalSpec`, `pub enum ColumnType` (and most variants), etc.
- `src/cli.rs`: `pub struct Cli`, `pub enum Commands`, all arg structs and their fields
- `src/join.rs`: `pub fn execute`, `pub enum JoinKind`, `pub struct JoinArgs`

**Suggested Resolution**: Sweep all `pub` items in each module and add concise `///` one-line summaries. Struct fields used in serde deserialization benefit especially from docs as they appear in generated help text. As a pragmatic starting point, add `#![deny(missing_docs)]` to `lib.rs` gated by `#[cfg(doc)]` to make this a gate on doc builds.

---

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

#### RI-01 — FIXED ✅
`Value::Ord` heterogeneous panic removed. Fallback now uses `variant_index().cmp()` at `data.rs:291`. `Float` comparison upgraded to `total_cmp` at `data.rs:283`.

#### RI-02 — FIXED ✅
`unsafe { from_utf8_unchecked }` replaced with safe `from_utf8(...).map_err(...)` at `io_utils.rs:197–198`. Zero unsafe code remains in the codebase.

#### RI-03 — FIXED ✅
Three `.expect()` calls in `schema_cmd.rs` production code replaced:
- L231: `.context("Preview requires serialized YAML output")?`
- L276: `.context("Diff requires serialized YAML output")?`
- L431: `.ok_or_else(|| anyhow!("Column '{column_name}' not found in schema"))?`

#### RI-04 — FIXED ✅
`partial_cmp().unwrap()` in `stats.rs:349` replaced with `total_cmp` (NaN-safe, stable since Rust 1.62).

#### RI-05 — FIXED ✅
Two `.expect()` calls in `frequency.rs:154,158` replaced with `.context(...)` + `?`.

#### RI-06 — FIXED ✅
Four `.expect()` calls in `schema.rs` replaced:
- L718: `.context("FixedDecimalValue produced invalid decimal spec")?`
- L2281: `.context("datatype_mappings is empty despite has_mappings() check")?`
- L2298: `.context("datatype_mappings is empty despite non-empty check")?`
- L2372: `.context("mapping chain must have terminal type")?`

#### RI-07 — FIXED ✅
`//!` module-level documentation added to `columns.rs`, `install.rs`, and `join.rs`.

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

## Quality Gate Results

| Gate | Command | Result |
|------|---------|--------|
| Build | `cargo build` | ✅ 0 errors |
| Lint | `cargo clippy --all-targets --all-features -- -D warnings` | ✅ 0 warnings |
| Tests | `cargo test --all-targets --all-features` | ✅ 229 passed, 1 ignored, 0 failed |
| Format | `cargo fmt --check` | ✅ no diffs |
| Missing docs | `RUSTFLAGS="-W missing-docs" cargo build` | ⚠️ 216 warnings (not a hard gate yet) |

## Next Steps

* [x] Phase 3: All RI items reviewed — all 7 resolved in current code
* [x] Run quality gates
* [x] Phase 4: Handoff document created
* [x] NEW-01 fixed: removed `.unwrap()` from schema.rs:1751, replaced with panic-free `let` chain — also removes unnecessary `Vec<char>` allocation
* [ ] Owner decision: roadmap plan for NEW-02 (216 missing `///` Rustdoc comments)
