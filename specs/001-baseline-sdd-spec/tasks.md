# Tasks: CSV-Managed — Baseline SDD Specification

**Input**: Design documents from `/specs/001-baseline-sdd-spec/`
**Prerequisites**: plan.md (required), spec.md (required), research.md, data-model.md, contracts/cli-contract.md, quickstart.md

**Tests**: Included — the baseline spec requires validating existing test coverage against all 59 functional requirements and 10 user stories.

**Organization**: Tasks are grouped by user story to enable independent validation and gap-filling for each story. Since this is a baseline spec for an *existing* implementation, tasks focus on: (1) validating existing code against the spec, (2) adding missing Rustdoc, (3) filling test coverage gaps, and (4) documenting edge-case behavior.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., US1, US2)
- Exact file paths included in descriptions

---

## Phase 1: Setup (SDD Alignment Infrastructure)

**Purpose**: Establish spec-driven development artifacts and validate project health

- [x] T001 Verify project builds cleanly with `cargo build --release` and `cargo test --all`
- [x] T002 [P] Verify `cargo clippy --all-targets --all-features -- -D warnings` produces zero warnings
- [x] T003 [P] Verify `cargo fmt --check` passes with no formatting diffs
- [x] T004 [P] Validate all spec artifacts exist: plan.md, spec.md, research.md, data-model.md, contracts/cli-contract.md, quickstart.md in specs/001-baseline-sdd-spec/

---

## Phase 2: Foundational (Cross-Cutting Validation)

**Purpose**: Validate shared infrastructure that ALL user stories depend on — data types, I/O, error handling, observability

**CRITICAL**: These validations must pass before story-level validation begins.

### Data Type System (FR-012 through FR-016)

- [x] T005 Audit `ColumnType` enum in src/schema.rs — confirm all 10 variants (String, Integer, Float, Boolean, Date, DateTime, Time, Guid, Currency, Decimal) exist and match FR-012
- [x] T006 [P] Audit boolean parsing in src/data.rs — confirm all 6 input formats (true/false, yes/no, 1/0, t/f, y/n) per FR-013 are handled
- [x] T007 [P] Audit date parsing in src/data.rs — confirm common formats (YYYY-MM-DD, MM/DD/YYYY) per FR-014 are handled and canonicalize to YYYY-MM-DD
- [x] T008 [P] Audit currency parsing in src/data.rs — confirm symbols ($, €, £, ¥), thousands separators, and parentheses notation per FR-015 are handled
- [x] T009 [P] Audit decimal parsing in src/data.rs — confirm precision/scale validation (max 28) and rounding strategies per FR-016

### I/O & Encoding (FR-051 through FR-054)

- [x] T010 Audit delimiter auto-detection in src/io_utils.rs — confirm extension-based detection (.csv→comma, .tsv→tab) and manual override per FR-051
- [x] T011 [P] Audit encoding support in src/io_utils.rs — confirm encoding_rs infrastructure exists for independent input/output encoding per FR-052 (end-to-end pipeline behavior validated in US7 T101)
- [x] T012 [P] Audit stdin/stdout support in src/io_utils.rs — confirm `-i -` reader infrastructure and stdout writer exist per FR-053 (end-to-end pipeline behavior validated in US7 T100)
- [x] T013 [P] Audit CSV output quoting in src/io_utils.rs — confirm all fields are quoted per FR-054

### Observability (FR-056 through FR-059)

- [x] T014 Audit timing output in src/lib.rs `run_operation()` — confirm structured start/end/duration per FR-056
- [x] T015 [P] Audit log verbosity in src/lib.rs `init_logging()` — confirm RUST_LOG support per FR-057
- [x] T016 [P] Audit operation outcome logging in src/lib.rs — confirm success/error with context per FR-058
- [x] T017 [P] Audit exit codes in src/main.rs — confirm 0 for success and non-zero for error per FR-059

### Rustdoc Gaps

- [x] T018 [P] Add/verify module-level Rustdoc comment in src/data.rs documenting Value enum, type parsing responsibilities, and complexity
- [x] T019 [P] Add/verify module-level Rustdoc comment in src/schema.rs documenting Schema model, inference, and YAML I/O
- [x] T020 [P] Add/verify module-level Rustdoc comment in src/io_utils.rs documenting encoding, delimiter, reader/writer utilities
- [x] T021 [P] Add/verify module-level Rustdoc comment in src/lib.rs documenting crate root, command dispatch, and timing wrapper
- [x] T145 [P] Add/verify module-level Rustdoc comment in src/process.rs documenting process command, sort strategy, and transform pipeline
- [x] T146 [P] Add/verify module-level Rustdoc comment in src/schema_cmd.rs documenting schema subcommand dispatch and probe/infer/verify orchestration
- [x] T147 [P] Add/verify module-level Rustdoc comment in src/cli.rs documenting clap argument structures and subcommand definitions
- [x] T148 [P] Add/verify module-level Rustdoc comment in src/main.rs documenting entry point and error handling
- [x] T149 [P] Add/verify module-level Rustdoc comment in src/derive.rs documenting derived column specification and evaluation
- [x] T150 [P] Add/verify module-level Rustdoc comment in src/rows.rs documenting row parsing and filter evaluation helpers
- [x] T151 [P] Add/verify module-level Rustdoc comment in src/table.rs documenting ASCII table renderer

### Foundational Test Coverage (FR-012–FR-016, FR-056–FR-059)

- [x] T152 [P] Verify tests exist for data type parsing success and failure paths (boolean formats, date formats, currency symbols, decimal precision overflow) covering FR-012 through FR-016 in tests/process.rs or tests/schema.rs
- [x] T153 [P] Verify tests exist for observability features (timing output, log verbosity, exit codes) covering FR-056 through FR-059 in tests/cli.rs

**Checkpoint**: Foundation validated — all shared infrastructure confirmed against spec.

---

## Phase 3: User Story 1 — Schema Discovery & Inference (Priority: P1) MVP

**Goal**: Validate that `schema probe` and `schema infer` fully implement FR-001 through FR-011.

**Independent Test**: Run `schema probe` and `schema infer` against test fixtures and verify output matches expected column structure and types.

### Validation for User Story 1

- [x] T022 [US1] Audit schema inference sampling in src/schema.rs — confirm configurable sample rows (default 2000, 0 = full scan) per FR-001
- [x] T023 [US1] Audit header detection in src/schema.rs — confirm auto-detection and synthetic name assignment (`field_0`, `field_1`) per FR-002
- [x] T024 [P] [US1] Audit `--assume-header` flag in src/cli.rs `SchemaProbeArgs` — confirm presence and behavior per FR-003
- [x] T025 [P] [US1] Audit schema YAML persistence in src/schema.rs `Schema::save()` — confirm column order, types, renames, mappings, replacements per FR-004
- [x] T026 [P] [US1] Audit schema probing in src/schema_cmd.rs — confirm read-only inference table display per FR-005
- [x] T027 [P] [US1] Audit unified diff in src/schema_cmd.rs — confirm diff output between inferred and existing schema per FR-006
- [x] T028 [P] [US1] Audit snapshot support in src/schema_cmd.rs — confirm SHA-256 hash of header/type layout per FR-007
- [x] T029 [P] [US1] Audit `--override` flag in src/cli.rs and src/schema_cmd.rs — confirm type override per FR-008
- [x] T030 [P] [US1] Audit NA-placeholder detection in src/schema.rs — confirm NA, N/A, #N/A, #NA, null, none handling with configurable behavior per FR-009
- [x] T031 [P] [US1] Audit manual schema creation in src/schema_cmd.rs — confirm `--column name:type` definitions per FR-010
- [x] T032 [P] [US1] Audit `--mapping` flag in src/schema_cmd.rs — confirm mapping scaffold and snake_case suggestions per FR-011

### Test Coverage for User Story 1

- [x] T033 [P] [US1] Verify test exists for acceptance scenario 1 (probe displays inference table) in tests/probe.rs
- [x] T034 [P] [US1] Verify test exists for acceptance scenario 2 (infer writes YAML schema) in tests/schema.rs
- [x] T035 [P] [US1] Verify test exists for acceptance scenario 3 (headerless CSV inference) in tests/schema.rs
- [x] T036 [P] [US1] Verify test exists for acceptance scenario 4 (NA-placeholder normalization) in tests/schema.rs or tests/probe.rs
- [x] T037 [P] [US1] Verify test exists for acceptance scenario 5 (schema diff) in tests/schema.rs
- [x] T038 [P] [US1] Verify test exists for acceptance scenario 6 (snapshot hash) in tests/schema.rs
- [x] T039 [US1] Add missing tests for any US1 acceptance scenarios not covered above

**Checkpoint**: Schema Discovery & Inference validated — all FR-001 through FR-011 confirmed.

---

## Phase 4: User Story 2 — Data Transformation & Processing (Priority: P1)

**Goal**: Validate that `process` fully implements FR-017 through FR-028.

**Independent Test**: Run `process` with filters, derives, sort, and column selection against known CSVs and verify output.

### Validation for User Story 2

- [x] T040 [US2] Audit row-level filtering in src/filter.rs — confirm all operators (=, !=, >, <, >=, <=, contains, startswith, endswith) per FR-017
- [x] T041 [P] [US2] Audit expression-based filtering in src/expr.rs — confirm boolean logic (AND, OR, nested if) per FR-018
- [x] T042 [P] [US2] Audit column projection in src/process.rs — confirm `--columns` include and `--exclude-columns` per FR-019
- [x] T043 [P] [US2] Audit derived columns in src/derive.rs and src/process.rs — confirm expression evaluation per FR-020
- [x] T044 [P] [US2] Audit multi-column sorting in src/process.rs — confirm per-column ascending/descending per FR-021
- [x] T045 [P] [US2] Audit datatype mapping application in src/process.rs — confirm ordered chain before replacements per FR-022
- [x] T046 [P] [US2] Audit value replacement application in src/process.rs — confirm schema-defined replacements per FR-023
- [x] T047 [P] [US2] Audit row number injection in src/process.rs — confirm `--row-numbers` per FR-024
- [x] T048 [P] [US2] Audit boolean format normalization in src/process.rs — confirm original/true-false/one-zero per FR-025
- [x] T049 [P] [US2] Audit row limit in src/process.rs — confirm `--limit` per FR-026
- [x] T050 [P] [US2] Audit preview mode in src/process.rs — confirm formatted table output per FR-027
- [x] T051 [P] [US2] Audit table mode in src/process.rs and src/table.rs — confirm elastic-width ASCII table per FR-028

### Test Coverage for User Story 2

- [x] T052 [P] [US2] Verify test for filter (acceptance scenario 1: `amount >= 100`) in tests/process.rs
- [x] T053 [P] [US2] Verify test for derive (acceptance scenario 2: computed column) in tests/process.rs
- [x] T054 [P] [US2] Verify test for index-accelerated sort (acceptance scenario 3) in tests/cli.rs or tests/process.rs
- [x] T055 [P] [US2] Verify test for column projection + exclusion (acceptance scenario 4) in tests/process.rs
- [x] T056 [P] [US2] Verify test for boolean normalization (acceptance scenario 5) in tests/process.rs
- [x] T057 [P] [US2] Verify test for preview mode (acceptance scenario 6) in tests/preview.rs
- [x] T058 [P] [US2] Verify test for value replacements (acceptance scenario 7) in tests/process.rs
- [x] T059 [P] [US2] Verify test for datatype mappings (acceptance scenario 8) in tests/process.rs
- [x] T060 [US2] Add missing tests for any US2 acceptance scenarios not covered above

**Checkpoint**: Data Transformation & Processing validated — all FR-017 through FR-028 confirmed.

---

## Phase 5: User Story 3 — Schema Verification (Priority: P1)

**Goal**: Validate that `schema verify` fully implements FR-041 through FR-044.

**Independent Test**: Run `schema verify` against CSVs with known invalid rows and confirm correct violation reports.

### Validation for User Story 3

- [x] T061 [US3] Audit cell-level type validation in src/verify.rs — confirm every cell checked against declared type per FR-041
- [x] T062 [P] [US3] Audit tiered reporting in src/verify.rs — confirm summary/detail modes and configurable limits per FR-042
- [x] T063 [P] [US3] Audit header mismatch detection in src/verify.rs — confirm CSV vs schema header comparison per FR-043
- [x] T064 [P] [US3] Audit multi-file verification in src/schema_cmd.rs — confirm independent per-file reporting per FR-044

### Test Coverage for User Story 3

- [x] T065 [P] [US3] Verify test for summary report (acceptance scenario 1: invalid cell counts) in tests/schema.rs
- [x] T066 [P] [US3] Verify test for detail report (acceptance scenario 2: row/column violations) in tests/schema.rs
- [x] T067 [P] [US3] Verify test for header mismatch (acceptance scenario 3) in tests/schema.rs
- [x] T068 [P] [US3] Verify test for multi-file verification (acceptance scenario 4) in tests/schema.rs
- [x] T069 [P] [US3] Verify test for capped detail report (acceptance scenario 5: limit) in tests/schema.rs
- [x] T070 [US3] Add missing tests for any US3 acceptance scenarios not covered above

**Checkpoint**: Schema Verification validated — all FR-041 through FR-044 confirmed.

---

## Phase 6: User Story 4 — B-Tree Indexing for Sort Acceleration (Priority: P2)

**Goal**: Validate that `index` and index-accelerated `process` fully implement FR-034 through FR-040.

**Independent Test**: Build an index, run `process` with matching sort, verify output order.

### Validation for User Story 4

- [x] T071 [US4] Audit B-Tree index build in src/index.rs — confirm byte-offset keys per FR-034
- [x] T072 [P] [US4] Audit multi-variant support in src/index.rs `CsvIndex` — confirm multiple named variants per FR-035
- [x] T073 [P] [US4] Audit covering expansion in src/index.rs `IndexDefinition::expand_covering_spec()` — confirm direction/prefix permutations per FR-036
- [x] T074 [P] [US4] Audit best-match selection in src/process.rs — confirm longest prefix match per FR-037
- [x] T075 [P] [US4] Audit `--index-variant` pinning in src/process.rs — confirm named variant selection per FR-038
- [x] T076 [P] [US4] Audit versioned binary format in src/index.rs `CsvIndex::save()`/`load()` — confirm version field and incompatibility detection per FR-039
- [x] T077 [US4] Audit streaming indexed sort in src/process.rs — confirm seek-based row reads without full buffering per FR-040

### Test Coverage for User Story 4

- [x] T078 [P] [US4] Verify test for named variant index build (acceptance scenario 1) in tests/cli.rs
- [x] T079 [P] [US4] Verify test for multi-spec index (acceptance scenario 2) in tests/cli.rs
- [x] T080 [P] [US4] Verify test for covering expansion (acceptance scenario 3) in tests/cli.rs
- [x] T081 [P] [US4] Verify test for partial match selection (acceptance scenario 4) in tests/process.rs or tests/cli.rs
- [x] T082 [P] [US4] Verify test for missing variant error (acceptance scenario 5) in tests/cli.rs
- [x] T083 [US4] Add missing tests for any US4 acceptance scenarios not covered above

**Checkpoint**: B-Tree Indexing validated — all FR-034 through FR-040 confirmed.

---

## Phase 7: User Story 5 — Summary Statistics & Frequency Analysis (Priority: P2)

**Goal**: Validate that `stats` fully implements FR-045 through FR-047.

**Independent Test**: Run `stats` and `stats --frequency` against known CSVs and verify metrics.

### Validation for User Story 5

- [x] T084 [US5] Audit summary statistics in src/stats.rs — confirm count, min, max, mean, median, stddev for numeric/temporal per FR-045
- [x] T085 [P] [US5] Audit frequency analysis in src/frequency.rs — confirm top-N distinct values with counts and percentages per FR-046
- [x] T086 [P] [US5] Audit filtered statistics in src/stats.rs — confirm filter application before computing per FR-047

### Test Coverage for User Story 5

- [x] T087 [P] [US5] Verify test for numeric summary (acceptance scenario 1) in tests/stats.rs
- [x] T088 [P] [US5] Verify test for temporal stats (acceptance scenario 2) in tests/stats.rs
- [x] T089 [P] [US5] Verify test for frequency top-N (acceptance scenario 3) in tests/stats.rs
- [x] T090 [P] [US5] Verify test for filtered stats (acceptance scenario 4) in tests/stats.rs
- [x] T091 [P] [US5] Verify test for decimal/currency precision in stats (acceptance scenario 5) in tests/stats.rs
- [x] T092 [US5] Add missing tests for any US5 acceptance scenarios not covered above

**Checkpoint**: Summary Statistics & Frequency validated — all FR-045 through FR-047 confirmed.

---

## Phase 8: User Story 6 — Multi-File Append (Priority: P2)

**Goal**: Validate that `append` fully implements FR-048 through FR-050.

**Independent Test**: Append multiple CSVs and verify unified output.

### Validation for User Story 6

- [x] T093 [US6] Audit multi-file append in src/append.rs — confirm header-once concatenation per FR-048
- [x] T094 [P] [US6] Audit header consistency check in src/append.rs — confirm mismatch error per FR-049
- [x] T095 [P] [US6] Audit schema-driven validation in src/append.rs — confirm type checking during append per FR-050

### Test Coverage for User Story 6

- [x] T096 [P] [US6] Verify test for identical-header append (acceptance scenario 1) in tests/cli.rs
- [x] T097 [P] [US6] Verify test for header mismatch error (acceptance scenario 2) in tests/cli.rs
- [x] T098 [P] [US6] Verify test for schema-validated append (acceptance scenario 3) in tests/cli.rs
- [x] T099 [US6] Add missing tests for any US6 acceptance scenarios not covered above

**Checkpoint**: Multi-File Append validated — all FR-048 through FR-050 confirmed.

---

## Phase 9: User Story 7 — Streaming Pipeline Support (Priority: P2)

**Goal**: Validate stdin/stdout pipeline composition per FR-053 and acceptance scenarios.

**Independent Test**: Pipe `process` output into `stats` using `-i -` and verify correct results.

### Validation for User Story 7

- [x] T100 [US7] Audit end-to-end stdin pipeline reading in src/process.rs — confirm `-i -` data flows correctly through process command to produce valid output per FR-053
- [x] T101 [P] [US7] Audit end-to-end encoding transcoding in piped commands — confirm `--input-encoding` / `--output-encoding` produce correctly transcoded output per FR-052
- [x] T102 [P] [US7] Audit preview mode behavior in piped context in src/process.rs — confirm table output (not CSV) per acceptance scenario 3

### Test Coverage for User Story 7

- [x] T103 [P] [US7] Verify test for `process | stats` pipeline (acceptance scenario 1) in tests/stdin_pipeline.rs
- [x] T104 [P] [US7] Verify test for encoding transcoding (acceptance scenario 2) in tests/cli.rs or tests/preview.rs
- [x] T105 [P] [US7] Verify test for preview mode in pipeline (acceptance scenario 3) in tests/stdin_pipeline.rs
- [x] T106 [US7] Add missing tests for any US7 acceptance scenarios not covered above

**Checkpoint**: Streaming Pipeline Support validated — FR-053 confirmed.

---

## Phase 10: User Story 8 — Expression Engine (Priority: P3)

**Goal**: Validate expression engine implements FR-029 through FR-033.

**Independent Test**: Run `process` with `--derive` and `--filter-expr` and verify computed values.

### Validation for User Story 8

- [x] T107 [US8] Audit temporal helper functions in src/expr.rs — confirm all 11 functions (date_add, date_sub, date_diff_days, date_format, datetime_add_seconds, datetime_diff_seconds, datetime_format, datetime_to_date, datetime_to_time, time_add_seconds, time_diff_seconds) per FR-029
- [x] T108 [P] [US8] Audit string functions in src/expr.rs — confirm `concat` per FR-030
- [x] T109 [P] [US8] Audit conditional logic in src/expr.rs — confirm `if(cond, true, false)` per FR-031
- [x] T110 [P] [US8] Audit positional aliases in src/expr.rs — confirm c0, c1 column resolution per FR-032
- [x] T111 [P] [US8] Audit `row_number` exposure in src/expr.rs — confirm availability when `--row-numbers` enabled per FR-033

### Test Coverage for User Story 8

- [x] T112 [P] [US8] Verify test for date_diff_days derive (acceptance scenario 1) in tests/process.rs
- [x] T113 [P] [US8] Verify test for compound filter expression (acceptance scenario 2) in tests/process.rs
- [x] T114 [P] [US8] Verify test for concat derive (acceptance scenario 3) in tests/process.rs
- [x] T115 [P] [US8] Verify test for row_number in expression (acceptance scenario 4) in tests/process.rs
- [x] T116 [P] [US8] Verify test for positional aliases (acceptance scenario 5) in tests/process.rs
- [x] T117 [US8] Add missing tests for any US8 acceptance scenarios not covered above

**Checkpoint**: Expression Engine validated — all FR-029 through FR-033 confirmed.

---

## Phase 11: User Story 9 — Schema Column Listing (Priority: P3)

**Goal**: Validate `schema columns` displays formatted column table.

**Independent Test**: Run `schema columns -m schema.yml` and verify output.

### Validation for User Story 9

- [x] T118 [US9] Audit columns command in src/columns.rs — confirm position, name, datatype, rename display

### Test Coverage for User Story 9

- [x] T119 [P] [US9] Verify test for schema columns table output (acceptance scenario 1) in tests/schema.rs or tests/cli.rs
- [x] T154 [P] [US9] Verify test for schema columns with renames (acceptance scenario 2) in tests/schema.rs or tests/cli.rs
- [x] T120 [US9] Add missing tests for any US9 acceptance scenarios not covered above

**Checkpoint**: Schema Column Listing validated.

---

## Phase 12: User Story 10 — Self-Install (Priority: P3)

**Goal**: Validate `install` wraps `cargo install` per FR-055.

**Independent Test**: Run `install --version X.Y.Z` and verify cargo command.

### Validation for User Story 10

- [x] T121 [US10] Audit install command in src/install.rs — confirm version, force, locked, root options per FR-055

### Test Coverage for User Story 10

- [x] T122 [P] [US10] Verify test for `install --locked` (acceptance scenario 1) in tests/cli.rs
- [x] T123 [P] [US10] Verify test for `install --version` (acceptance scenario 2) in tests/cli.rs
- [x] T124 [US10] Add missing test if US10 acceptance scenarios not covered

**Checkpoint**: Self-Install validated — FR-055 confirmed.

---

## Phase 13: Polish & Cross-Cutting Concerns

**Purpose**: Edge cases, documentation completeness, and overall SDD alignment

### Edge Case Validation

- [ ] T125 [P] Verify behavior for empty CSV file (0 bytes) across schema probe, process, stats, verify
- [ ] T126 [P] Verify behavior for header-only CSV (no data rows) across stats and verify
- [ ] T127 [P] Verify behavior for unknown column in filter expression — confirm clear error message
- [ ] T128 [P] Verify behavior for malformed derive expression — confirm parse error with position
- [ ] T129 [P] Verify behavior for empty stdin pipe — confirm detection and reporting
- [ ] T130 [P] Verify behavior for decimal precision overflow (>28 digits) — confirm error
- [ ] T131 [P] Verify behavior for column rename with original header name — confirm transparent mapping
- [ ] T132 [P] Verify behavior for multiple `--filter` flags — confirm AND semantics
- [ ] T133 [P] Verify behavior for `--sort` without matching index on large input — confirm in-memory fallback

### Documentation Completeness

- [ ] T134 [P] Add/verify Rustdoc for all public types in src/index.rs (CsvIndex, IndexVariant, IndexDefinition, SortDirection)
- [ ] T135 [P] Add/verify Rustdoc for all public types in src/filter.rs (ComparisonOperator, FilterCondition)
- [ ] T136 [P] Add/verify Rustdoc for all public types in src/expr.rs (expression context, temporal functions)
- [ ] T137 [P] Add/verify Rustdoc for all public types in src/verify.rs (verification engine, report types)
- [ ] T138 [P] Add/verify Rustdoc for all public types in src/append.rs (append execution)
- [ ] T139 [P] Add/verify Rustdoc for all public types in src/stats.rs and src/frequency.rs

### Constitution Compliance

- [ ] T155 [P] Audit failure-path test coverage for all public parsers — confirm each has at least one failure test per Constitution Testing Strategy
- [ ] T156 [P] Audit hot-path modules (src/data.rs, src/process.rs, src/schema.rs) for unnecessary String allocations or cloning — confirm Zero-Copy / Borrowing principle per Constitution Principle III

### Final Validation

- [ ] T140 Run full `cargo test --all` — confirm all existing and new tests pass
- [ ] T141 Run `cargo clippy --all-targets --all-features -- -D warnings` — confirm zero warnings
- [ ] T142 Run `cargo doc --no-deps` — confirm Rustdoc builds without warnings
- [ ] T143 Run quickstart.md examples against test fixtures — validate documented workflows
- [ ] T144 Cross-reference all 59 functional requirements (FR-001 through FR-059) against task completions — confirm 100% coverage

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies — start immediately
- **Foundational (Phase 2)**: Depends on Phase 1 — BLOCKS all user story phases
- **User Stories (Phases 3–12)**: All depend on Phase 2 completion
  - P1 stories (Phases 3, 4, 5) can proceed in parallel
  - P2 stories (Phases 6, 7, 8, 9) can proceed in parallel (recommended after P1 for context, but independently validatable)
  - P3 stories (Phases 10, 11, 12) can proceed in parallel (recommended after P2 for context, but independently validatable)
- **Polish (Phase 13)**: Depends on all user stories being validated

### User Story Dependencies

- **US1 (Schema Discovery)**: Independent — no cross-story dependencies
- **US2 (Processing)**: Independent — uses schema but validates processing logic in isolation
- **US3 (Verification)**: Independent — uses schema but validates verification logic in isolation
- **US4 (Indexing)**: Independent — index build is self-contained; index-sort uses process but validates index logic
- **US5 (Statistics)**: Independent — validates stats computation in isolation
- **US6 (Append)**: Independent — validates append logic in isolation
- **US7 (Pipeline)**: May use US2 (process) output but validates stdin/stdout plumbing independently
- **US8 (Expressions)**: Independent — validates expression engine functions in isolation
- **US9 (Schema Columns)**: Independent — validates display command in isolation
- **US10 (Self-Install)**: Independent — validates install wrapper in isolation

### Parallel Opportunities

- All `[P]` tasks within a phase can run simultaneously
- All audit tasks within a user story phase can run simultaneously
- All test verification tasks within a user story phase can run simultaneously
- Multiple user stories at the same priority level can be worked in parallel

---

## Parallel Example: User Story 1

```text
# Launch all audit tasks for US1 together:
T023 Audit header detection in src/schema.rs
T024 Audit --assume-header flag in src/cli.rs
T025 Audit schema YAML persistence in src/schema.rs
T026 Audit schema probing in src/schema_cmd.rs
T027 Audit unified diff in src/schema_cmd.rs
T028 Audit snapshot support in src/schema_cmd.rs
T029 Audit --override flag in src/cli.rs and src/schema_cmd.rs
T030 Audit NA-placeholder detection in src/schema.rs
T031 Audit manual schema creation in src/schema_cmd.rs
T032 Audit --mapping flag in src/schema_cmd.rs

# Then launch all test verification tasks together:
T033 Verify test for probe inference table
T034 Verify test for infer writes YAML
T035 Verify test for headerless CSV
T036 Verify test for NA-placeholder
T037 Verify test for schema diff
T038 Verify test for snapshot hash
```

---

## Implementation Strategy

### Approach: Validation-First

Since this is a baseline spec for an existing implementation:

1. **Phase 1–2**: Verify build health and foundational types/I/O — establish green baseline
2. **Phase 3 (US1)**: Validate the most fundamental story first (schema discovery)
3. **Phases 4–5 (US2, US3)**: Validate the other P1 stories in parallel
4. **Phases 6–9 (US4–US7)**: Validate P2 stories — these build on the P1 foundation
5. **Phases 10–12 (US8–US10)**: Validate P3 stories — convenience and advanced features
6. **Phase 13**: Edge cases, Rustdoc gaps, and final cross-reference

### Per-Story Workflow

For each user story:
1. Audit source files against the mapped functional requirements
2. Flag any implementation gaps (unexpected — this is an existing system)
3. Verify test coverage against acceptance scenarios
4. Add missing tests for uncovered scenarios
5. Add Rustdoc where missing
6. Mark story checkpoint as complete

### MVP Scope

Phase 1 (Setup) + Phase 2 (Foundational) + Phase 3 (US1: Schema Discovery) constitute the minimum viable validation. After completing these, the baseline spec can be considered partially validated with the core story confirmed.

---

## FR → Task Traceability

| FR Range | Story | Validation Tasks | Test Tasks |
|----------|-------|------------------|------------|
| FR-001–FR-011 | US1 | T022–T032 | T033–T039 |
| FR-012–FR-016 | Foundation | T005–T009 | T152 |
| FR-017–FR-028 | US2 | T040–T051 | T052–T060 |
| FR-029–FR-033 | US8 | T107–T111 | T112–T117 |
| FR-034–FR-040 | US4 | T071–T077 | T078–T083 |
| FR-041–FR-044 | US3 | T061–T064 | T065–T070 |
| FR-045–FR-047 | US5 | T084–T086 | T087–T092 |
| FR-048–FR-050 | US6 | T093–T095 | T096–T099 |
| FR-051–FR-054 | Foundation + US7 | T010–T013, T100–T102 | T103–T106 |
| FR-055 | US10 | T121 | T122–T124 |
| FR-056–FR-059 | Foundation | T014–T017 | T153 |

---

## Notes

- `[P]` tasks = different files, no dependencies between them
- `[Story]` label maps task to specific user story for traceability
- Each user story is independently validatable
- "Audit" tasks verify existing code against the spec — no new implementation expected
- "Verify test" tasks confirm existing test coverage — "Add missing" tasks fill gaps
- Commit after each completed user story phase
- Edge case tasks (T125–T133) validate the spec's Edge Cases section
