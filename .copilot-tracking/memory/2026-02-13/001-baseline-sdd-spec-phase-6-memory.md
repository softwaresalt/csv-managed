# Session Memory: 001-baseline-sdd-spec — Phase 6

**Date**: 2026-02-13
**Spec**: specs/001-baseline-sdd-spec/
**Phase**: 6 — User Story 4: B-Tree Indexing for Sort Acceleration (P2)
**Branch**: 001-baseline-sdd-spec

## Task Overview

Phase 6 validates User Story 4 (B-Tree Indexing for Sort Acceleration)
covering FR-034 through FR-040. 13 tasks total (T071–T083): 7 source code
audits and 6 test coverage verifications.

## Current State

### All 13 tasks completed

| Task Range | Category | Outcome |
|---|---|---|
| T071 | B-Tree index build (FR-034) | PASS — `CsvIndex::build()` uses `BTreeMap<Vec<DirectionalComparableValue>, Vec<u64>>` keyed by concatenated typed column values, storing byte offsets |
| T072 | Multi-variant support (FR-035) | PASS — `CsvIndex` stores `variants: Vec<IndexVariant>`; `build()` accepts `&[IndexDefinition]` and builds all variants in a single CSV pass; `variant_by_name()` supports named lookup |
| T073 | Covering expansion (FR-036) | PASS — `IndexDefinition::expand_covering_spec()` parses `name=col:asc\|desc,col2:asc`, generates all direction/prefix permutations via `cartesian_product()` with named prefix |
| T074 | Best-match selection (FR-037) | PASS — `CsvIndex::best_match()` iterates variants, calls `variant.matches()` (prefix match), and selects the variant with the longest matching column set |
| T075 | `--index-variant` pinning (FR-038) | PASS — `process.rs` reads `args.index_variant`, calls `index.variant_by_name(name)`, validates sort match, returns clear error if variant not found |
| T076 | Versioned binary format (FR-039) | PASS — `INDEX_VERSION = 2`; `save()` serializes with `bincode`, `load()` checks version and returns error on mismatch; `LegacyCsvIndex` fallback for v1 format |
| T077 | Streaming indexed sort (FR-040) | PASS — `ProcessEngine::process_with_index()` iterates `variant.ordered_offsets()`, seeks per byte offset, reads single records without full-file buffering; bucket sub-sort for partial coverage |
| T078 | Test: named variant build (AS1) | COVERED — `process_accepts_named_index_variant` in tests/cli.rs builds two named specs and uses `--index-variant recent` |
| T079 | Test: multi-spec index (AS2) | COVERED — same test builds with two `--spec` flags; unit test `build_multiple_variants_and_match` also validates |
| T080 | Test: covering expansion (AS3) | COVERED — `index_covering_spec_generates_multiple_variants` in tests/cli.rs uses `--covering geo=ordered_at:asc\|desc,amount:asc` and asserts >= 4 variants with `geo_` prefix |
| T081 | Test: partial match selection (AS4) | COVERED — unit test `build_multiple_variants_and_match` + new `best_match_selects_longest_prefix_variant` for true prefix-longer selection; integration test `process_with_index_respects_sort_order` |
| T082 | Test: missing variant error (AS5) | COVERED — `process_errors_when_variant_missing` in tests/cli.rs asserts failure with "Index variant 'missing' not found" |
| T083 | Add missing US4 tests | Added 2 unit tests: `best_match_selects_longest_prefix_variant` (FR-037 prefix selection gap) and `load_rejects_incompatible_index_version` (FR-039 version detection gap) |

### Files Modified

| File | Change |
|---|---|
| src/index.rs | Added 2 unit tests: `best_match_selects_longest_prefix_variant`, `load_rejects_incompatible_index_version` |
| specs/001-baseline-sdd-spec/tasks.md | Marked T071–T083 as complete |

### Test Results

- 96 unit tests: all pass
- 28 CLI integration tests: all pass
- 6 preview tests: all pass
- 3 probe tests: all pass
- 18 process tests: all pass
- 23 schema tests: all pass
- 8 stats tests: all pass
- 5 stdin pipeline tests: 4 pass, 1 ignored (expected)
- Clippy: zero warnings
- Rustfmt: clean

### No ADRs Created

No significant architectural decisions were made. All tasks were
validation/audit of existing code confirming correct FR-034 through
FR-040 implementation.

## Important Discoveries

- The `CsvIndex` version field is mutable (not `pub` but accessible
  within the module), enabling the version incompatibility test via
  direct field manipulation before save.
- The `best_match` algorithm uses a simple linear scan with longest-wins
  strategy. For large variant counts, this remains O(n*k) where n is the
  number of variants and k is the sort column count. Adequate for expected
  variant counts (typically < 20).
- The `LegacyCsvIndex` fallback transparently upgrades v1 single-variant
  indexes to the v2 multi-variant format with all-ascending directions.

## Next Steps

- **Phase 7** (US5: Summary Statistics & Frequency Analysis, FR-045–FR-047):
  Audit `src/stats.rs` and `src/frequency.rs` against statistical
  computation requirements.
- **Phase 8** (US6: Multi-File Append, FR-048–FR-050): Audit `src/append.rs`
  header consistency and schema-driven validation.
- **Phase 9** (US7: Streaming Pipeline Support, FR-053): Audit stdin/stdout
  pipeline composition end-to-end.

## Context to Preserve

- **Source files**: `src/index.rs` (818 LOC → 867 LOC with new tests),
  `src/process.rs` (783 LOC, unmodified)
- **Test files**: `tests/cli.rs` (997 LOC, unmodified — all 5 index-related
  integration tests pre-existed), `tests/process.rs` (1315 LOC, unmodified —
  `process_with_index_respects_sort_order` pre-existed)
- **Index format**: `INDEX_VERSION = 2`, bincode legacy config, `LegacyCsvIndex`
  migration path for v1
