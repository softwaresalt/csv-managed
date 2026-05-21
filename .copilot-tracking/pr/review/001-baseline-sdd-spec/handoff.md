<!-- markdownlint-disable-file -->
# PR Review Handoff: 001-baseline-sdd-spec

## PR Overview

Full baseline codebase implementation of csv-managed — a high-performance streaming CSV CLI tool.
This PR introduced all 21 source modules (~10,700 LOC) covering the complete feature set:
schema inference/verification, process (filter/sort/project/derive), stats, frequency, index,
append, join (staged), encoding, and self-install.

* Branch: `001-baseline-sdd-spec` (merged to main)
* Base Branch: `main`
* Total Files Changed: 21 source files + test suite
* Total Review Comments: 9 items identified (7 prior RI-01–RI-07; 2 new NEW-01–NEW-02)

---

## Quality Gate Summary

| Gate | Result |
|------|--------|
| `cargo build` | ✅ Clean |
| `cargo clippy --all-targets --all-features -- -D warnings` | ✅ Zero warnings |
| `cargo test --all-targets --all-features` | ✅ 229 passed, 1 ignored |
| `cargo fmt --check` | ✅ No diffs |
| `RUSTFLAGS="-W missing-docs" cargo build` | ⚠️ 216 warnings (see NEW-02) |

---

## Prior Review Items — Resolution Status

All seven items identified in the previous review attempt have been **fully resolved** in the current codebase.

| Item | Description | File | Status |
|------|-------------|------|--------|
| RI-01 | `Value::Ord` panics on heterogeneous variants | `src/data.rs:291` | ✅ FIXED |
| RI-02 | `unsafe { from_utf8_unchecked }` | `src/io_utils.rs:197` | ✅ FIXED |
| RI-03 | 3× `.expect()` in schema_cmd.rs production code | `src/schema_cmd.rs:231,276,431` | ✅ FIXED |
| RI-04 | `partial_cmp().unwrap()` in stats.rs median | `src/stats.rs:349` | ✅ FIXED |
| RI-05 | 2× `.expect()` in frequency.rs | `src/frequency.rs:154,158` | ✅ FIXED |
| RI-06 | 4× `.expect()` in schema.rs | `src/schema.rs:718,2281,2298,2372` | ✅ FIXED |
| RI-07 | Missing `//!` module docs in columns.rs, install.rs, join.rs | 3 files | ✅ FIXED |

### Fix Details

**RI-01** (`src/data.rs`): The `Ord` impl for `Value` now delegates heterogeneous comparisons to `self.variant_index().cmp(&other.variant_index())` (line 291), providing a deterministic total order instead of panicking. Float comparisons were upgraded from `partial_cmp().unwrap()` to `total_cmp()` (line 283), making NaN handling correct and explicit.

**RI-02** (`src/io_utils.rs`): The sole `unsafe` block has been removed. The `TranscodingWriter::flush_complete()` method now uses the safe `std::str::from_utf8(valid_slice).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?` (lines 197–198). The codebase is now entirely `unsafe`-free.

**RI-03** (`src/schema_cmd.rs`): All three `.expect()` calls in the production dispatch path are replaced with proper `anyhow` error propagation:
- Preview path (L231): `.context("Preview requires serialized YAML output")?`
- Diff path (L276): `.context("Diff requires serialized YAML output")?`
- Column lookup (L431): `.ok_or_else(|| anyhow!("Column '{column_name}' not found in schema"))?`

**RI-04** (`src/stats.rs:349`): Median sort now uses `a.total_cmp(b)` — stable since Rust 1.62, handles NaN deterministically (sorts NaN as greater than all finite values), and removes the panic path entirely.

**RI-05** (`src/frequency.rs:154,158`): HashMap access guards now propagate errors via `.context("Column should exist in totals")?` and `.context("Column should exist in counts")?`.

**RI-06** (`src/schema.rs`): All four previously-`.expect()`ed sites now use `.context(...)?` or `anyhow!` propagation, maintaining the error chain for diagnostics without panicking.

**RI-07**: Module-level `//!` documentation added to `src/columns.rs`, `src/install.rs`, and `src/join.rs`.

---

## PR Comments Ready for Submission

### File: `src/schema.rs`

#### Comment 1 (Line 1751)

* Category: Convention / Code Quality
* Severity: LOW

**Issue**: The private function `build_header_aliases()` contains a `.unwrap()` call in production code that violates the project convention of no `unwrap()`/`expect()` in library code.

```rust
// Current (line 1749–1754):
if sanitized.len() >= 2 {
    let chars: Vec<char> = sanitized.chars().collect();
    let first = chars.first().copied().unwrap();  // ← unwrap in production code
    let last = chars.last().copied().unwrap_or(first);
    let shorthand = format!("{}{}", first, last);
    try_insert(&shorthand);
}
```

While this is logically infallible (the `!sanitized.is_empty()` guard on line 1747 ensures `chars` is non-empty before the `len() >= 2` check runs), the code still violates the stated convention and would alarm future contributors performing a `grep` audit for `unwrap()`. The intermediate `Vec<char>` allocation also adds minor unnecessary heap pressure on every call to `build_header_aliases()`, which is called once per header column during schema inference.

**Suggested Change**:

```rust
if sanitized.len() >= 2 {
    // Avoid Vec<char> allocation; chars() iterator is sufficient.
    // Both unwraps are safe: sanitized.len() >= 2 guarantees at least 2 chars.
    let first = sanitized.chars().next().expect("sanitized.len() >= 2");
    let last = sanitized.chars().next_back().unwrap_or(first);
    try_insert(&format!("{first}{last}"));
}
```

> ⚠️ If the project constitution strictly prohibits `.expect()` as well as `.unwrap()`, use the idiomatic `if let` form instead:
> ```rust
> if sanitized.len() >= 2 {
>     if let (Some(first), Some(last)) = (
>         sanitized.chars().next(),
>         sanitized.chars().next_back(),
>     ) {
>         try_insert(&format!("{first}{last}"));
>     }
> }
> ```

---

### Files: All public-facing modules

#### Comment 2 (Project-wide — 216 items)

* Category: Documentation
* Severity: LOW

**Issue**: Running `RUSTFLAGS="-W missing-docs" cargo build` emits **216 warnings** for public items that lack `///` Rustdoc comments. The project constitution states "All public items must have `///` Rustdoc comments." The current state covers module-level `//!` docs (all 21 modules) but not per-item inline docs.

Breakdown:
- 49 struct fields missing docs
- 46 methods missing docs
- 36 functions missing docs
- 35 enum variants missing docs
- 27 structs missing docs
- 11 associated functions missing docs
- 9 enums missing docs
- 3 constants missing docs

Most-affected files (by item count):
1. `src/cli.rs` — arg structs and their fields (affects `--help` output quality)
2. `src/data.rs` — `FixedDecimalValue`, `CurrencyValue`, `Value`, and all public parse functions
3. `src/schema.rs` — `DecimalSpec`, `ColumnType` variants, `Schema`, `ColumnMeta`, `DatatypeMapping`
4. `src/io_utils.rs` — all 10+ public functions
5. `src/join.rs` — `JoinKind`, `JoinArgs`, `execute()`

**Suggested Change**: Enforce this convention as a hard gate on documentation builds by adding to `src/lib.rs`:

```rust
#![cfg_attr(doc, deny(missing_docs))]
```

This gates the lint on `cargo doc` builds only (not debug/release), preserving a clean `cargo clippy` without a noisy incremental transition. Then sweep modules in priority order: `cli.rs` (affects user-facing help), `data.rs` (core type API), `io_utils.rs` (called from every module), then `schema.rs`.

Representative minimal docs for the highest-traffic items:

```rust
// src/data.rs
/// Parses a raw CSV string cell into a typed [`Value`] for the given column type.
/// Returns `Ok(None)` for empty input, `Ok(Some(value))` on success, or an error.
pub fn parse_typed_value(value: &str, ty: &ColumnType) -> Result<Option<Value>> { … }

/// Converts a [`Value`] to its `evalexpr` counterpart for expression evaluation.
pub fn value_to_evalexpr(value: &Value) -> evalexpr::Value { … }
```

```rust
// src/io_utils.rs
/// Opens a seekable CSV reader backed by a real file for index-accelerated reads.
/// Returns an error if the path does not exist or cannot be opened.
pub fn open_seekable_csv_reader(…) -> Result<csv::Reader<BufReader<File>>> { … }
```

---

## Deferred Items (Non-Blocking)

These were evaluated and deferred from this review; they are recorded here for future roadmap planning.

| Item | Reason for Deferral | Recommendation |
|------|---------------------|----------------|
| **stats.rs unbounded `Vec<f64>`** for median | Streaming median requires significant algorithmic work (P-squared algorithm or reservoir sampling); correctness risk high if rushed | Track as separate issue; document current O(n) memory bound in `ColumnStats` struct doc |
| **join.rs right-side fully in memory** | Hash-join semantics require right-side materialization; design is documented in module `//!` comment | Add a `log::warn!` when right-side row count exceeds a configurable threshold (e.g., 1M rows) |
| **join.rs command not yet dispatched** | `Commands::Join` and `cli.rs` `Join` variant both commented out pending integration testing | Tracked; no CLI exposure until tests are added |
| **schema.rs 3,195 LOC** | Splitting into submodules carries refactoring risk; no functional issue | Consider `schema/inference.rs`, `schema/mapping.rs`, `schema/serde.rs` split in a follow-up PR |
| **process.rs `execute()` > 200 lines** | Refactoring risk outweighs style benefit for this PR; passes all tests | Extract `run_indexed_sort()` and `run_streaming_pass()` helpers in a separate PR |

---

## Review Summary by Category

| Category | Count | Items |
|----------|-------|-------|
| ✅ Security | 1 fixed | RI-02 (unsafe removed) |
| ✅ Correctness | 1 fixed | RI-01 (Ord panic) |
| ✅ Convention (unwrap/expect) | 4 fixed | RI-03, RI-04, RI-05, RI-06 |
| ✅ Documentation (module `//!`) | 1 fixed | RI-07 |
| ⚠️ Convention (production `unwrap`) | 1 open | NEW-01 |
| ⚠️ Documentation (`///` per-item) | 1 open | NEW-02 (216 items) |
| — Deferred | 5 items | See Deferred Items table |

---

## Instruction Compliance

| Instruction File | Status | Notes |
|-----------------|--------|-------|
| `.github/instructions/rust.instructions.md` | ⚠️ Partial | `unsafe` ✅ eliminated; `unwrap/expect` in lib code ✅ eliminated (one minor production `unwrap` remains at schema.rs:1751); `///` Rustdoc ⚠️ 216 items still missing |
| `.github/copilot-instructions.md` | ✅ Met | Streaming architecture maintained; `anyhow::Result` throughout; `println!` restricted to CLI boundary |
| `specs/001-baseline-sdd-spec/plan.md` | ✅ Met | No `unsafe`, no unwrap/expect in core library paths, streaming CSV honored |

---

## Outstanding Risks

1. **`stats.rs` memory growth for large datasets** — the median accumulator holds all numeric values in RAM. A 100M-row file with 20 numeric columns could require ~16 GB. This is the most significant operational risk for production workloads. Consider adding a `--no-median` flag or switching to an approximate streaming algorithm.

2. **`join.rs` right-side memory** — the right-side file is fully materialized into a `HashMap`. For multi-GB right files this will OOM. Since `join` is not yet CLI-dispatched, this is pre-production risk.

3. **Docs gap at API surface** — 216 missing `///` docs mean auto-generated API documentation is sparse. This is a quality-of-life issue for contributors and library consumers, not a runtime risk.
