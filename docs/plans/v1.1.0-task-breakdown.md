# v1.1.0 Implementation Task Breakdown

This file enumerates the full, ordered, implementable task list derived from `backlog.md` (v1.1.0 section) and `feature-plan.md`. Tasks are grouped into phases (F1–F6) aligning with feature plan items plus cross‑cutting quality gates. Each task is intentionally granular to enable focused PRs, clear review scope, and measurable progress.

Legend: (P*) = prerequisite for later tasks, (Doc) = documentation output, (Test) = testing work, (CI) = CI/CD integration, (Spike) = exploratory.

---

## Phase F1: YAML Backend Spike & Abstraction

Goal: Evaluate alternative YAML crates and introduce an abstraction to reduce future migration cost.

1. (Spike) Define evaluation criteria table: performance, error reporting, API ergonomics, maintenance, spec compliance.
2. (Spike) Create temporary benchmark harness for representative schema load/save (small & medium fixtures).
3. (Spike) Add minimal prototypes using `serde_yaml_ng`, `serde_yaml_ok`, `serde_yml` behind isolated branches or feature flags.
4. Capture error message samples (malformed YAML) for each candidate; record line/column fidelity.
5. Measure serialization/deserialization timing & memory (rough, using `std::time::Instant` + heap track via `dh-attribution` or manual approximation).
6. (P*) Introduce `src/yaml_provider.rs` with `YamlProvider` trait (`load_from_path`, `save_to_path`).
7. Implement default provider (initially wrapping existing `serde_yaml`).
8. Add unit tests for provider abstraction (success + malformed YAML error propagation).
9. Refactor schema load/save call sites to route through provider (grep for direct `serde_yaml` usage).
10. Run full test suite to confirm no regressions.
11. (Doc) Update `README.md` architecture section to mention YAML provider abstraction.

## Phase F2: Test Refactor & Shared Helpers

Goal: Improve maintainability & enable reliable coverage metrics.

1. Inventory current inline `#[cfg(test)]` modules in `src/**`; list file->new test file mapping.
2. Create `tests/common/mod.rs` with: `TestContext`, temp file helpers, `CommandExt` trait (from feature plan).
3. Migrate module tests from `src/*` into integration test files (e.g., `tests/schema_cmd.rs`, etc.).
4. Ensure no duplicated test logic; remove now-empty inline test modules.
5. Add fixture loader helper for schema & CSV fixtures (centralize path resolution).
6. Add negative tests for malformed schema YAML (uses provider abstraction from F1).
7. Activate previously ignored evolution test placeholder (will fail until F4 implemented).
8. (P*) Confirm `cargo test` green post-migration.
9. (Doc) Update `CONTRIBUTING` or add `docs/testing.md` describing new layout + helper usage.

## Phase F3: Coverage Harness & CI Integration

Goal: Establish baseline observable coverage.

1. Add dev tooling notes in `docs/testing.md` for `cargo llvm-cov` usage.
2. Add coverage script (PowerShell + Bash) in `scripts/coverage.(ps1|sh)` to run llvm-cov and emit `lcov.info`.
3. Update CI workflow (`.github/workflows/ci.yml` or create if absent) with coverage job (report-only, non-blocking).
4. Publish `lcov.info` as artifact + integrate Codecov or Coveralls action.
5. Parse baseline coverage; store summary in `docs/coverage/baseline.md`.
6. Add optional `fail-under` step commented for future activation (document threshold policy 80% line / 70% branch).
7. (Doc) Add badge placeholder and instructions for enabling threshold.

## Phase F4: Schema Evolution Emission

Goal: Deterministic diff artifacts enabling evolution audits.

1. Define `SchemaChangeKind` enum + `SchemaChange` struct + `SchemaEvolution` container (sorted output rules).
2. Implement diff algorithm: load prior schema + current inferred schema → emit ordered changes.
3. Add CLI flag `schema evolution --previous <path>` or `schema infer --evolution-from <path>` (choose least disruptive; document rationale).
4. Emit evolution artifact `<schema_base>.evo.yml` (separate file) in deterministic column + change-kind order.
5. Add formatting & serialization tests (stable ordering, empty diff case, multiple change kinds).
6. Unignore evolution test (from F2 step 19); ensure pass with new implementation.
7. Add README section “Schema Evolution” + cross-link from schema command docs.
8. Add `docs/schema-evolution.md` with examples (added/removed/renamed/datatype changed).
9. (CI) Ensure evolution tests included in coverage metrics.
10. (Doc) Update `docs/adr.md` if evolution requires structural decision notes.

## Phase F5: String Transformation Framework

Goal: Extend row transformation expressiveness.

1. Implement helper module `src/transform/string_ops.rs` with zero/low-allocation functions: `lowercase`, `uppercase`, `snake_case`, `trim`, `substring(start,len)`, `regex_replace(pattern,replacement)` using `Cow<'_, str>`.
2. Integrate transforms into expression engine (register evalexpr custom functions).
3. Add validation: substring bounds, graceful empty regex match behavior; return errors via unified Error type.
4. Add unit tests for each transform (already transformed input, edge cases: empty string, Unicode, multi-byte substrings).
5. Add integration tests using `process --derive` and `--filter` with transforms.
6. (Perf) Optional micro-bench for `snake_case` vs naive implementation (deferred if cost minimal).
7. (Doc) Update `docs/expressions.md` with transform usage examples.
8. Add examples to `docs/examples.ps1` / `.bat` demonstrating chained transforms.
9. Implement `camel_case` transform: word boundary detection from underscores, hyphens, spaces; first word lowercased, subsequent words capitalized; preserve internal Unicode letters without ASCII-only assumptions.
10. Implement `pascal_case` transform: same tokenization as camel_case; all words capitalized; ensure correct handling of leading numerics (leave numerics untouched) and acronyms (e.g., "APIResponse" rule: treat each token individually).
11. Add Unicode normalization utilities: select normalization form (flag `--unicode-normalize {nfc|nfkc|nfd|nfkd}` or expression functions `normalize_nfc(str)`, etc.) using `unicode-normalization` crate behind `unicode_norm` feature flag.
12. Integrate normalization functions into expression engine (register `normalize_nfc`, `normalize_nfkc`, `normalize_nfd`, `normalize_nfkd`).
13. Add unit tests for camel_case & pascal_case covering: simple ASCII, mixed delimiters (`foo-bar_baz qux` → `fooBarBazQux` / `FooBarBazQux`), existing camelCase input (idempotence), all caps acronym input (`HTTP_STATUS`), leading/trailing delimiters, multi-byte characters (e.g., `café_price` → `cafePrice` with NFC normalization optional).
14. Add unit tests for Unicode normalization: composed vs decomposed (`é` vs `é`), emoji sequences, CJK characters (should remain stable), idempotence across repeated normalization calls.
15. Add integration tests combining case transforms + normalization in derive expressions (e.g., `derive:new_col=pascal_case(normalize_nfc(original))`).
16. Update error handling: ensure invalid normalization form yields descriptive CLI error and expression function validation error.
17. Add performance micro-bench (optional) comparing normalization + case transform pipeline vs naive String rebuild for medium length (128–256 char) inputs.
18. (Doc) Expand `docs/expressions.md` with camel_case, pascal_case, and normalization examples including edge cases and performance note.
19. Add examples to `docs/examples.ps1` / `.bat` demonstrating pipeline: `snake_case` → `camel_case` → `normalize_nfc`.
20. Add feature flag documentation for `unicode_norm` in README (purpose, enabling, expected impact) and ensure default build excludes crate unless explicitly enabled.

## Phase F6: Auditable Snapshot Enhancements

Goal: Provide structured, metadata-rich snapshot artifacts.

1. Define `Snapshot`, `SnapshotMetadata`, `SnapshotColumn` structs in `src/schema_cmd.rs` or dedicated `snapshot.rs`.
2. Add CLI flags: `--snapshot-format (text|json)` default text; `--snapshot-notes <string>`.
3. Refactor existing snapshot generator to populate struct then render text OR serialize JSON.
4. Add metadata fields: timestamp (ISO 8601), crate version, source file path, full invoked command, notes.
5. Ensure header hash generation reused for both formats; test determinism (stable inputs → identical hash).
6. Add tests: json format correctness, notes injection, deterministic ordering of columns.
7. Add sample snapshot artifacts under `tests/data/snapshots/` for regression comparison.
8. (Doc) Update README snapshot section + add `docs/snapshots-and-verification.md` subsection for JSON format.
9. Ensure coverage includes new snapshot code paths.

## Cross-Cutting Quality & Maintenance

1. Run `cargo clippy --all-targets --all-features -D warnings` after each major phase; fix warnings immediately.
2. Maintain CHANGELOG entries per phase (F1–F6) with concise bullet summaries.
3. Add performance note if YAML backend selection changes parsing latency (record benchmark deltas in `docs/perf/yaml_backend.md`).
4. Verify Windows + Linux local runs for transforms & evolution outputs (line endings consistency test).
5. Ensure all new public items have Rustdoc (include complexity notes where non-trivial).
6. Re-run full test suite with `-- --test-threads=1` once to check hidden concurrency assumptions.
7. Add security review for regex usage (document pattern injection considerations).
8. Confirm no panics introduced; audit with `grep -R "panic!" src`.
9. Finalize coverage threshold activation (uncomment CI fail-under) once baseline stabilized.

## Release Preparation (Post-Implementation)

1. Update version to v1.1.0 in `Cargo.toml`.
2. Generate release notes summarizing features (F1–F6) + quality improvements.
3. Smoke test CLI: evolution, string transforms, snapshot JSON, existing core commands unaffected.
4. Tag release and verify binary build artifacts.
5. Publish documentation updates; ensure badges (coverage placeholder) visible.

## ADR Alignment Tasks

Goal: Ensure implementation strictly reflects accepted architecture decisions (ADR-001, ADR-002, ADR-003) and transitions their status from Proposed → Accepted.

1. Update ADR-001 status to Accepted; append implementation notes (dependency switch date, crate version pinned).
2. Modify `Cargo.toml` to replace `serde_yaml` with `serde_yaml_ng` (pin minimal version); remove legacy dependency.
3. Add feature flag `yaml_fallback_serde_yaml` (optional) documented in README for emergency rollback; default disabled.
4. Add malformed YAML fixture (`tests/data/malformed_schema.yml`) to assert improved line/column error messages.
5. Add integration test `tests/yaml_errors.rs` verifying error context (line & column) via provider abstraction.
6. Record benchmark comparison results in `docs/perf/yaml_backend.md` (previous vs new crate) referencing ADR-001.
7. Update ADR-002 status to Accepted; add note of llvm-tools-preview installation command.
8. Extend coverage script to check presence of `llvm-tools-preview`; fail with actionable message if missing.
9. Update CI workflow to install component: `rustup component add llvm-tools-preview` before coverage job.
10. Pin `cargo-llvm-cov` binary version (document chosen version and update policy in `docs/testing.md`).
11. Add CONTRIBUTING section describing local coverage setup & troubleshooting (link ADR-002).
12. Update ADR-003 status to Accepted; confirm naming convention `<schema_base>.evo.yml` documented.
13. Add test `tests/schema_evolution_artifact.rs` ensuring schema file remains unchanged and separate evolution artifact produced.
14. Add README subsection clarifying evolution artifact lifecycle & commit guidance (link ADR-003).
15. Add `.evo.yml` inclusion guidance to version control (ensure NOT in `.gitignore`).
16. Create maintenance script `scripts/check_adr_status.(ps1|sh)` that scans ADR files ensuring no lingering Proposed decisions without corresponding tasks.
17. Run script in CI (non-blocking initially) to report ADR status summary.
18. Update release notes template to include “ADR Status Changes” section listing accepted ADRs.
19. Add rustdoc references (module-level) pointing to relevant ADR IDs for yaml provider and evolution diff modules.
20. Validate all ADR alignment tasks completed before toggling coverage threshold enforcement.

## Validation Checklist (Completion Gate)

- Evolution diff deterministic across two consecutive runs.
- Snapshot JSON round-trips in provider abstraction (load + reserialize stable).
- String transforms produce expected outputs for ASCII + Unicode samples.
- Coverage >= baseline target (document exact % in coverage summary).
- All tasks in phases F1–F6 marked complete in tracking.

## Deferred / Future Considerations (Not In Scope v1.1.0)

- Embedding evolution data directly into schema (`--embed-evolution`).
- Full benchmark integration gating PRs.
- Primary key indexing (scheduled for future milestone).

---
