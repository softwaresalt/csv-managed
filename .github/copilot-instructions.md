# Copilot & Contributor Instructions for `csv-managed`

This document guides AI-assisted and human contributions for a high‑performance Rust command-line tool that manages very large CSV/TSV datasets (hundreds of GB+) for data engineering, data science, and ML workflows. It establishes coding, testing, performance, and release practices so generated or manual changes remain consistent, robust, and memory‑efficient.

## Core Goals
1. Stream, transform, validate, and index tabular datasets with minimal memory footprint.
2. Provide schema-driven guarantees (types, aliases, renames, primary/composite keys, currency/decimal precision, temporal parsing, candidate key probing).
3. Support batch pipelines, unions, deduplication, splitting, indexing, statistics, verification, and schema inference at scale.
4. Maintain predictable performance characteristics across platforms (Windows, macOS, Linux) and Rust stable toolchains.

## High-Level Architecture Principles
| Principle | Rationale | Guidance |
|----------|-----------|----------|
| Streaming / Iterators | Avoid loading entire files | Favor `csv::Reader` with `byte_records()` or `records()`; wrap in lazy adapters. |
| Separation of Concerns | Simplify maintenance | Distinct modules: parsing, schema, indexing, stats, filtering, expressions, CLI. |
| Zero-Copy / Borrowing | Reduce allocations | Prefer `&str` / slices, avoid unnecessary `String` cloning, use `Cow<'_, str>` if conditional ownership. |
| Explicit Error Types | Improves debuggability | Use custom enums with `thiserror` or manual `Display` impl; wrap lower-level errors. |
| Deterministic Performance | Reproducible runs | Avoid hidden global state; gate costly features behind flags. |
| Extensibility via Traits | Future column operations | Define trait abstractions for transforms and validators. |
| Config-First | Batch + repeatable runs | Support JSON pipeline definition and YAML schema as canonical inputs. |

## Rust Coding Standards
1. Rust Edition: Use latest stable edition (update `Cargo.toml` only after CI passes on stable/nightly). 
2. Formatting: Enforce `rustfmt` defaults—do not manually reflow unless readability improves semantics.
3. Linting: Treat `cargo clippy --all-targets --all-features -D warnings` as mandatory pre-merge.
4. Error Handling:
	- Never silently discard errors. Propagate with `?` unless recovery is required.
	- Use `Result<T, E>` for fallible operations; prefer `Option<T>` only when absence is expected and non-error.
5. Avoid premature `unsafe`. If unavoidable, isolate in a single module with comments: invariants, preconditions, UB avoidance.
6. Favor explicit lifetimes only when the compiler cannot infer.
7. Limit macro usage to reducing boilerplate (e.g., repetitive enum conversions); prefer functions and traits.
8. Document public items with Rustdoc including invariants, complexity (Big‑O), and error cases.
9. Use clear naming: `snake_case` for fields/functions; `PascalCase` for types; avoid abbreviations.
10. Keep function bodies ideally <100 LOC; refactor logic into internal helpers for clarity.

## Data Engineering Specific Patterns
1. Large File Handling:
	- Use streaming; don’t collect entire column sets unnecessarily.
	- Chunk operations (e.g., indexing) with bounded buffers configurable via CLI.
2. Type Normalization:
	- Centralize parse logic: implement a `DataType` enum with associated `parse(&str) -> Result<Value, ParseError>`.
	- Maintain currency precision (≤ 4 decimals) via a `Decimal` wrapper (e.g., `rust_decimal::Decimal`).
3. Schema Application:
	- Resolve alias mapping early; maintain both original and canonical name maps.
	- Validate column presence + type before heavy transforms.
4. Candidate Key Probing:
	- Sample first N rows + reservoir sample thereafter for large files.
	- Track uniqueness via fast `FxHashSet` or `hashbrown::HashSet` keyed on concatenated normalized values or a row hash.
5. Row Hashing / Indexes:
	- Use stable hashing (e.g., `ahash` or `twox-hash`) gated behind feature flags for reproducibility concerns.
	- Persist indexes with versioned headers (magic bytes + semantic version + hash algo identifier).
6. Unions & Deduplication:
	- Maintain canonical ordering of columns based on schema; insert missing columns as null/default.
	- Deduplicate using a streaming set membership strategy; for very large sets, consider on-disk bloom or partitioning.

## Memory & Performance Practices
1. Profile early using `cargo bench` and `criterion` for critical operations (scan, index build, union). 
2. Use `cargo flamegraph` (feature gated in CI) for hotspots.
3. Prefer `&[u8]` operations for raw CSV lines, decoding only when required.
4. Minimize allocations: reuse buffers (e.g., one `String` scratch per thread). 
5. Consider parallelism with `rayon` only after confirming CPU-bound scenarios; avoid over-threading on I/O bound tasks.
6. Avoid broad `collect::<Vec<_>>()` on large iterators; if needed, annotate rationale.
7. SIMD / fast paths: Use crates (`simdutf8`, `lexical-core`) behind a `performance` feature flag.
8. Provide metrics counters (rows processed, parse failures, duplicates) optionally via a `--stats` flag.
9. Bench naming convention: `benches/<area>_<operation>.rs` (e.g., `index_vs_sort.rs`).

## Testing Strategy
| Test Type | Location | Purpose | Notes |
|----------|----------|---------|-------|
| Unit | `src/**` (inline module tests) | Validate small pure functions | Keep minimal fixtures inline. |
| Integration | `tests/*.rs` | Cross-module behavior, CLI flows | Use fixture loader helpers. |
| Property | `tests/` (feature: `proptest`) | Fuzz parsers, schema inference | Disable heavy cases in CI by default. |
| Snapshot | `tests/` with `insta` | Stable textual outputs (schema list, stats) | Redact volatile fields (timestamps). |
| Benchmark | `benches/` | Performance regression detection | Not run on every PR unless label `perf`. |

### Test Fixtures
1. Put large or reusable sample files under `tests/data/` or `tmp/` if ephemeral.
2. Keep fixture size small (< 50KB) for unit tests; larger (MB–GB) only for local performance validation.
3. Provide helper `fn load_fixture(name: &str) -> PathBuf` to standardize path resolution.
4. Use synthetic deterministic datasets for index and key probing tests (avoid randomness unless property testing).
5. For currency, include edge precision (0, 0.0001, 123456.9999, invalid forms). 

### Writing Tests
1. Always assert both success path and at least one failure path per public parser.
2. Use `assert_eq!` with descriptive messages or `pretty_assertions` (behind feature flag) for readability.
3. For CLI tests, use `assert_cmd` + `predicates` to validate stdout/stderr; avoid brittle full-line matches, prefer substring or structured JSON if available.
4. Ensure tests are independent; avoid global mutable state.
5. Mark slow tests with `#[ignore]` (attribute applied above test function) and document how to run them manually.

### Test Data Integrity
Add invariants comments: e.g., `// Invariant: first column is unique for candidate key detection test`.

## Error Handling & Logging
1. Central error enum (`Error`) with variants: `Io`, `Parse`, `Schema`, `Validation`, `Index`, `Cli`, `Other(String)`.
2. Use structured logging (e.g., `tracing`) with spans: `schema_load`, `index_build`, `union_execute`.
3. Log levels: `info` for progress, `debug` for internal decisions, `trace` for per-row diagnostics (guarded by feature `trace-rows`).
4. Never print directly to stdout/stderr from deep logic—bubble status up; CLI layer handles user messaging.

## Concurrency Guidelines
1. Only parallelize CPU-intensive transforms (e.g., hashing, type conversion) after profiling.
2. Guarantee deterministic output ordering when union or sort is requested (collect + stable sort or order-preserving merges).
3. Use channels sparingly; prefer iterator adaptors unless cross-thread streaming required.

## Schema & Type System
1. YAML schema: include: version, columns (name, alias?, datatype, nullable, precision?, format?), primary_key (list), transforms.
2. Provide CLI command to emit schema as markdown or list form (already supported—keep compatibility).
3. Column renames applied exactly once at ingestion boundary.
4. Validation steps order: (1) Header detection → (2) Column count check → (3) Rename/Alias mapping → (4) Type parsing → (5) Constraint checks (primary key uniqueness, currency precision) → (6) Optional transforms.

## CLI UX Guidelines
1. All commands must support `--help` with examples (see `docs/cli-help.md`).
2. Fail fast: invalid flags produce a concise error + suggest `--help`.
3. Provide dry-run modes (`--dry-run` or `--plan`) for destructive or heavy operations (unions, indexing, splits).
4. Support `--output -` (stdout) where feasible for piping.
5. Ensure exit codes: `0` success, `1` user error, `2` internal/unexpected (log details). 

## Feature Flags (Cargo)
| Feature | Purpose | Notes |
|---------|---------|-------|
| `performance` | SIMD & fast parsing | Optional; verify on stable. |
| `trace-rows` | Deep per-row logging | Disabled by default; avoid in benchmarks. |
| `benchmarks` | Criterion dependency | Not in production builds. |
| `proptest` | Property tests | CI optional tier. |

## Benchmarking & Profiling Workflow
1. Local: `cargo bench --features benchmarks`.
2. Flamegraph: `cargo flamegraph --bin csv-managed` (ensure `perf` / `dtrace` permissions).
3. Track median, mean, std-dev for hot paths; store historical results in `docs/perf/` (CSV).
4. Prefer relative comparisons (before vs after change) over absolute numbers across machines.

## Performance Review Checklist
- [ ] Streaming iteration (no full-file load)
- [ ] Minimal allocations (no large `Vec` unless justified)
- [ ] No unnecessary clones/hashes
- [ ] Bounded memory growth under worst-case file size
- [ ] Deterministic ordering when required
- [ ] Latency documented for large sample (e.g., 10M rows)

## CI / Quality Gates
1. Build matrix: stable + latest nightly (nightly only for future gating, no required success to merge).
2. Steps:
	- `cargo fmt --check`
	- `cargo clippy -D warnings`
	- `cargo test --all --features ""`
	- (Optional labeled perf runs) `cargo bench`
3. Cache: use GitHub Actions cache for `~/.cargo/registry` and `~/.cargo/git` keyed by Cargo.lock hash.
4. Security audit: `cargo audit` on schedule (weekly) + manual on release.
5. Release build: `cargo build --release` with `RUSTFLAGS='-C opt-level=3 -C codegen-units=1 -C strip=symbols'`.

## Release & Deployment
1. Semantic Versioning: MAJOR (breaking), MINOR (feature, backward-compatible), PATCH (fixes/perf, no behavior change).
2. Tag process: ensure CHANGELOG entry + updated README usage examples.
3. Provide prebuilt binaries (Windows x86_64, Linux x86_64/musl, macOS aarch64/x86_64) via GitHub Actions artifacts.
4. Use `cross` for multi-platform builds when native toolchains problematic.
5. After release: run smoke tests invoking core CLI commands on small fixtures.

## Documentation Standards
1. Each module: top-level comment summarizing responsibilities + complexity points.
2. Rustdoc examples must compile (`cargo test --doc`).
3. Keep `README.md` concise—defer deep examples to `docs/`.
4. Add ADRs (`docs/adr.md`) for major design decisions (index format, hashing strategy, currency representation).

## Observability & Diagnostics
1. Structured logs with context keys: `row_index`, `column_name`, `datatype`.
2. Optional stats output: JSON or tabular; stable schema for downstream automation.
3. Panic policy: avoid panics except unrecoverable invariants (document rationale).

## Copilot Prompting Guidance
When requesting AI-generated code:
- Specify: "streaming iterator for CSV rows" instead of generic "parse CSV".
- Include desired data structures (e.g., `HashMap<String, ColumnMeta>`).
- Mention constraints: memory cap, deterministic ordering, error propagation pattern (`Result<_, Error>`).
- Ask for tests simultaneously (happy path + failure case) to reduce omissions.
- For performance improvements, request microbench harness if complexity > trivial.

### Example Good Prompts
> Generate a function that applies schema type parsing to a single CSV record using a reusable scratch buffer; return a Vec<ParsedValue> or Error. Include unit tests for valid, invalid currency precision.
> Provide an iterator adapter that deduplicates rows based on a precomputed hash set; ensure memory stays bounded; add benchmark skeleton.

## Common Pitfalls to Avoid
1. Reading entire file into memory before transforming.
2. Using `String` where `&str` suffices.
3. Unbounded `HashSet` growth without documenting memory trade-offs.
4. Silent type coercion (e.g., trimming precision without logging). 
5. Inconsistent column rename order causing mismatched indexes.

## Edge Cases Checklist
- Empty file (0 bytes)
- Header only, no data rows
- Mixed line endings (CRLF vs LF)
- Quoted fields with embedded delimiter
- Invalid UTF-8 (fallback to lossy decode or error?)
- Extremely wide rows (thousands of columns)
- Currency with >4 decimals
- Date/time in multiple formats when schema expects one
- Duplicate primary key rows
- Missing required columns

## Adding New Features – Mini Contract Template
When introducing a new operation (e.g., column pivot):
1. Inputs: CLI args, schema requirements, file patterns.
2. Outputs: New file(s), index, stats.
3. Errors: Validation failure, I/O error, parse failure, constraint violation.
4. Performance target: Complexity analysis + memory notes.
Include this contract in PR description.

## Review Checklist (Pre-Merge)
- [ ] Rustdoc added/updated
- [ ] Tests pass & coverage for failure paths
- [ ] Clippy clean (`-D warnings`)
- [ ] Benchmark unaffected or improved (if modified hot path)
- [ ] No new `unsafe` or justified + documented
- [ ] CHANGELOG updated if user-visible behavior changed

## Suggested Future Enhancements
- Pluggable output formats (Parquet/Arrow) behind features
- Adaptive sampling for candidate key inference
- On-disk spill for dedupe when memory threshold exceeded
- Incremental index updates (append-only strategy)
- Parallel columnar type conversion pipeline

## Security & Safety
1. Validate paths to avoid directory traversal in batch definitions.
2. Avoid executing arbitrary expressions from user-provided schema.
3. Treat malformed CSV as recoverable where feasible; log and continue if configured.
4. Restrict currency transformation to documented rounding rules (bankers vs truncation—choose and document).

## Contributing Flow Summary
1. Create branch
2. Implement feature with tests
3. Run fmt, clippy, tests
4. Add docs / CHANGELOG
5. Open PR with feature contract & performance notes
6. Await review + potential benchmark validation

<!-- MANUAL ADDITIONS START -->

## Terminal Command Execution Policy

**Do NOT chain terminal commands.** Run each command as a separate, standalone invocation.

### Rules

1. **One command per terminal call.** Never combine commands with `;`, `&&`, `||`, or `|` unless it falls under an allowed exception below.
2. **No `cmd /c` wrappers.** Run commands directly in the shell rather than wrapping them in `cmd /c "..."`. If `cmd /c` is genuinely required (e.g., for environment isolation), it must contain a single command only.
3. **No exit-code echo suffixes.** Do not append `; echo "EXIT: $LASTEXITCODE"` or `&& echo "done"` to commands. The terminal tool already captures exit codes.
4. **Check results between commands.** After each command, inspect the output and exit code before deciding whether to run the next command. This is safer and produces better diagnostics.
5. **Always use `pwsh`, never `powershell`.** When invoking PowerShell explicitly (e.g., to run a `.ps1` script), use `pwsh` — the cross-platform PowerShell 7+ executable. Never use `powershell` or `powershell.exe`, which refers to the legacy Windows PowerShell 5.1 runtime.

### Allowed Exceptions

Output redirection is **not** command chaining — it is I/O plumbing that cannot execute destructive operations. The following patterns are permitted:

- **Shell redirection operators**: `>`, `>>`, `2>&1` (e.g., `cargo test > target/results.txt 2>&1`)
- **Pipe to `Out-File` or `Set-Content`**: `cargo test 2>&1 | Out-File target/results.txt` or `| Set-Content`
- **Pipe to `Out-String`**: `some-command | Out-String`

Use these when the terminal tool's ~60 KB output limit would truncate results (e.g., full `cargo test` compilation + test output).

### Why

Terminal auto-approve rules use regex pattern matching against the full command line. Chained commands create unpredictable command strings that cannot be reliably matched, forcing manual approval prompts that slow down the workflow. Single commands match cleanly and approve instantly.

### Correct Examples

```powershell
# Good: separate calls
cargo check
# (inspect output)
cargo clippy -- -D warnings
# (inspect output)
cargo test

# Good: output redirection to capture full results
cargo test 2>&1 | Out-File target\test-results.txt

# Good: shell redirect when output may be truncated
cargo test > target\test-results.txt 2>&1
```

### Incorrect Examples

```powershell
# Bad: chained with semicolons
cargo check; cargo clippy -- -D warnings; cargo test

# Bad: cmd /c wrapper with echo suffix
cmd /c "cargo test > target\test-results.txt 2>&1"; echo "EXIT: $LASTEXITCODE"

# Bad: AND-chained
cargo fmt && cargo clippy && cargo test

# Bad: pipe to something other than Out-File/Set-Content/Out-String
cargo test | Select-String "FAILED" | Remove-Item foo.txt
```
### Full List of Auto-Approve Commands with RegEx

"chat.tools.terminal.autoApprove": {
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
<!-- MANUAL ADDITIONS END -->
