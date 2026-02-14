---
name: build-feature
description: "Usage: Build feature {spec-name} phase {phase-number}. Implements a single phase from the spec's task plan, iterating build-test cycles until the phase passes its constitution gate, then records memory, logs decisions, and commits."
version: 1.0
maturity: stable
input:
  properties:
    spec-name:
      type: string
      description: "Directory name of the feature spec under specs/ (e.g., 001-batch-processing)."
    phase-number:
      type: integer
      description: "Phase number to build from the spec's tasks.md (e.g., 6 for Phase 6)."
  required:
    - spec-name
    - phase-number
---

# Build Feature Skill

Implements a single phase from a feature specification's task plan. The workflow iterates through build-test cycles until the phase satisfies its constitution gate, then records session memory, logs architectural decisions, and commits all changes.

## Prerequisites

* A feature spec directory exists at `specs/${input:spec-name}/` containing `plan.md`, `spec.md`, and `tasks.md`
* The target phase exists in `tasks.md` with defined tasks
* The project compiles before starting (`cargo check` passes)
* The `.github/agents/copilot-instructions.md` constitution and coding standards are accessible

## Quick Start

Invoke the skill with both required parameters:

```text
Build feature 001-<spec-name> phase <phase-number>
```

The skill runs autonomously through all required steps, halting only on unrecoverable errors or constitution violations requiring human judgment.

## Parameters Reference

| Parameter      | Required | Type    | Description                                                        |
| -------------- | -------- | ------- | ------------------------------------------------------------------ |
| `spec-name`    | Yes      | string  | Directory name under `specs/` containing the feature specification |
| `phase-number` | Yes      | integer | Phase number from `tasks.md` to implement                          |

## Required Steps

### Step 1: Load Phase Context

* Read `specs/${input:spec-name}/tasks.md` and extract all tasks for the specified phase.
* Read `specs/${input:spec-name}/plan.md` for architecture, tech stack, and project structure.
* Read `specs/${input:spec-name}/spec.md` for user stories and acceptance scenarios relevant to this phase.
* Read `specs/${input:spec-name}/data-model.md` if it exists, for entity definitions and relationships.
* Read `specs/${input:spec-name}/contracts/` if it exists, for API specifications and error codes.
* Read `specs/${input:spec-name}/research.md` if it exists, for technical decisions and constraints.
* Read `specs/${input:spec-name}/quickstart.md` if it exists, for integration scenarios.
* Read `.github/agents/copilot-instructions.md` for the project constitution, coding standards, and session memory requirements.
* Read `.github/agents/rust-engineer.agent.md` for language-specific engineering standards.
* Read `.github/instructions/rust.instructions.md` for general Rust coding conventions.
* Build a task execution list respecting dependencies: sequential tasks run in order, tasks marked `[P]` can run in parallel.
* Identify which tasks are tests and which are implementation; TDD order means test tasks execute before their corresponding implementation tasks.
* Report a summary of the phase scope: task count, estimated files affected, and user story coverage.

### Step 2: Check Constitution Gate

* Read `specs/${input:spec-name}/plan.md` and locate the Constitution Check table.
* Verify every principle listed in the table is satisfied for the work about to begin.
* If `specs/${input:spec-name}/checklists/` exists, scan all checklist files and for each checklist count:
  * Total items: all lines matching `- [ ]` or `- [X]` or `- [x]`
  * Completed items: lines matching `- [X]` or `- [x]`
  * Incomplete items: lines matching `- [ ]`
* Create a status table:

```text
| Checklist   | Total | Completed | Incomplete | Status |
|-------------|-------|-----------|------------|--------|
| ux.md       | 12    | 12        | 0          | PASS   |
| test.md     | 8     | 5         | 3          | FAIL   |
| security.md | 6     | 6         | 0          | PASS   |
```

* If any constitution principle is violated or any required checklist is incomplete, halt and report the violation with actionable remediation steps.
* If all gates pass, proceed to Step 3.

### Step 3: Build Phase (Iterative)

Execute tasks in dependency order following TDD discipline:

1. For each task group (tests first, then implementation):
   * Classify the task type to determine which coding constraints apply:
     * Schema tasks (touching `schema.rs`, `schema_cmd.rs`, `verify.rs`, `columns.rs`): Apply Schema and Error Handling constraints from the Coding Standards section below.
     * Processing tasks (touching `process.rs`, `filter.rs`, `derive.rs`, `rows.rs`): Apply Processing Pipeline and Error Handling constraints.
     * I/O tasks (touching `io_utils.rs`, `append.rs`): Apply I/O and Error Handling constraints.
     * Index tasks (touching `index.rs`): Apply Index and Error Handling constraints.
     * Expression tasks (touching `expr.rs`): Apply Expression Engine and Error Handling constraints.
     * CLI tasks (touching `cli.rs`, `lib.rs`): Apply CLI and Error Handling constraints.
   * Read any existing source files that the task modifies.
   * For test tasks: write the test first, then run it and **confirm the test fails** before implementing the production code (red-green TDD).
   * Implement the task following the coding standards from the rust-engineer agent, injecting only the task-type-specific constraints identified above.
   * After implementing each task, run `cargo check` to verify compilation.
   * If compilation fails, diagnose the error, fix it, and re-run `cargo check` until it passes.
   * A task is complete only when `cargo check` passes **and** relevant tests pass. Mark the completed task as `[X]` in `specs/${input:spec-name}/tasks.md`.

2. Follow these implementation rules:
   * Setup tasks first (project structure, dependencies, configuration).
   * Test tasks before their corresponding implementation tasks (TDD).
   * Respect `[P]` markers: parallel tasks touching different files can be implemented together.
   * Sequential tasks (no `[P]` marker) must complete in listed order.
   * Tasks affecting the same files must run sequentially regardless of markers.

3. Error handling during build:
   * Halt execution if any sequential task fails. Do not proceed to the next task until the failure is resolved.
   * For parallel tasks `[P]`, continue with successful tasks and report failed ones.
   * Provide clear error messages with context for debugging.
   * If implementation cannot proceed, report the blocker and suggest next steps.

4. Track architectural decisions made during implementation for recording in Step 6.

### Step 4: Test Phase (Iterative)

Run the full test suite and iterate until all tests pass:

1. Run `cargo test --all-targets --all-features` to execute all test suites.
2. If any test fails:
   * Diagnose the failure from the test output.
   * Fix the implementation (not the test, unless the test itself has a bug).
   * Re-run `cargo test --all-targets --all-features` to verify the fix.
   * Repeat until all tests pass.
3. Run `cargo clippy --all-targets --all-features -- -D warnings` to verify lint compliance.
4. If clippy reports warnings or errors, fix them and re-run until clean.
5. Run `cargo fmt --all -- --check` to verify formatting.
6. If formatting violations exist, run `cargo fmt --all` and verify.
7. Report final test results: suite counts, pass rates, and any notable findings.

Return to Step 3 if test failures reveal missing implementation work. Continue iterating between Step 3 and Step 4 until both build and test pass cleanly.

### Step 5: Constitution Validation

Re-check the constitution after implementation is complete:

* Verify no `unsafe` blocks were introduced.
* Verify no `unwrap()` or `expect()` calls exist in library code paths (test code is acceptable).
* Verify all new public items have `///` doc comments.
* Verify error handling uses `anyhow::Result` with `.with_context()` on fallible calls.
* Verify new `ColumnType` variants (if any) have corresponding `data::Value` variants and vice versa.
* Verify streaming code paths use forward-only CSV iteration and do not buffer entire datasets.
* If any violation is found, return to Step 3 to remediate before proceeding.

### Step 6: Record Architectural Decisions

For each significant decision made during the build phase:

* Create an ADR file in `docs/adrs/` following the naming convention `NNNN-{short-title}.md` where `NNNN` is the next sequential number (zero-padded to 4 digits).
* Each ADR includes:
  * Title describing the decision
  * Status (Accepted)
  * Context explaining the problem or situation
  * Decision made and rationale
  * Consequences (positive, negative, and risks)
  * Date and the phase/task that prompted the decision
* Decisions worth recording include: dependency choices, CLI design trade-offs, schema format changes, index format changes, expression engine extensions, streaming vs buffering trade-offs, and delimiter/encoding handling decisions.
* Skip this step if no significant architectural decisions were made during the phase.

### Step 7: Record Session Memory

Persist the full session details to `.copilot-tracking/memory/` following the project's session memory requirements:

* Create a memory file at `.copilot-tracking/memory/{YYYY-MM-DD}/{spec-name}-phase-{N}-memory.md` where the date is today and N is the phase number.
* The memory file includes:
  * Task Overview: phase scope and objectives
  * Current State: all tasks completed, files modified, test results
  * Important Discoveries: decisions made, failed approaches, CSV parsing or expression engine quirks encountered
  * Next Steps: what the next phase should address, any open questions, known issues
  * Context to Preserve: source file references, agent references, unresolved questions
* Use the existing memory files in `.copilot-tracking/memory/` as format examples.

### Step 8: Stage and Commit

1. Review all changes made during the phase to ensure they align with the completed tasks and constitution.
2. Review the ADRs created in Step 6 for clarity and completeness.
3. Review all steps to ensure that no steps have been missed and address any missing steps in the sequence before proceeding.
4. Review the session memory file for completeness and accuracy.

### Step 9: Stage, Commit, and Sync

Finalize all changes with a Git commit:

1. Accept all current diff changes (no interactive review).
2. Run `git add -A` to stage all modified, created, and deleted files.
3. Compose a commit message following these conventions:
   * Format: `feat({spec-name}): complete phase {N} - {phase title}`
   * Body: list of completed task IDs and a brief summary of what was built
   * Footer: reference the spec path and any relevant ADR numbers
4. Run `git commit` with the composed message.
5. Run `git push` to sync the commit to the remote repository.
6. Report the commit hash and a summary of changes committed.

### Step 10: Compact Conversation History

Compact the current session to create space in the session's context window.

1. Run `/compact` to compact the conversation history.

## Troubleshooting

### Tests pass locally but fail in CI

Verify the Rust toolchain matches CI configuration in `.github/workflows/ci.yml`. CI runs `cargo test --all-targets --all-features` and `cargo clippy --all-targets --all-features -- -D warnings`.

### Index deserialization errors

If `.idx` files fail to load after format changes, check that `INDEX_VERSION` in `index.rs` was incremented and old index files are regenerated.

### Delimiter or encoding issues in tests

Use `write_sample_csv(delimiter)` for temp files and `fixture_path(name)` for test data. Verify delimiter auto-detection matches the file extension (`.csv` → comma, `.tsv` → tab).

### Constitution violation detected

Return to Step 3 and fix the violation before proceeding. Common violations include `unwrap()` usage in library code, missing doc comments on public items, `unsafe` blocks, and buffering entire datasets in streaming code paths.

## Coding Standards

These rules are injected into each task based on its type classification in Step 3.

### General Rust

* Avoid `unsafe` code
* Prefer borrowing over cloning
* Use `anyhow::Result<()>` for all fallible functions; attach `.with_context()` at boundaries

### Error Handling

* `anyhow` is the primary error mechanism throughout binary and library code
* Use `anyhow!()` / `bail!()` for contextual errors
* Error messages should include relevant file paths or column names

### Schema

* Schema files are YAML (`*-schema.yml`), deserialized with `serde_yaml` into `Schema`
* `ColumnType` enum has 8 variants: String, Integer, Float, Boolean, Date, DateTime, Time, Guid
* `data::Value` mirrors `ColumnType` — keep them in sync when adding new types
* Schema supports column rename via `mapping`, per-column `replace` arrays, and `datatype_mappings`

### Processing Pipeline

* Streaming first: use forward-only CSV iteration, do not buffer entire datasets
* Index-assisted reads select the longest matching sort prefix to avoid in-memory sorting
* Normalization order: datatype_mappings → replace mappings → typed parse → filter → project → derive → write

### I/O

* All I/O flows through `io_utils` — delimiter resolution, encoding resolution, CSV reader/writer construction
* `encoding_rs` handles character encoding; default is UTF-8
* CSV writers use `QuoteStyle::Always` for quote safety
* Stdin/stdout streaming via `-` path convention

### Index

* Binary `.idx` files serialized with `bincode`, versioned via `INDEX_VERSION`
* Multi-variant B-tree supporting mixed asc/desc sort directions
* Increment `INDEX_VERSION` when changing the serialization format

### Expression Engine

* `evalexpr` crate with temporal helper functions registered in `expr.rs`
* Shared by `--derive` and `--filter-expr` via `build_context()` and `evaluate_expression_to_bool()`
* New helper functions must be registered in `register_temporal_functions()`

### CLI

* Defined via `clap` derive macros in `cli.rs`
* Each subcommand has its own `*Args` struct
* `preprocess_cli_args` in `lib.rs` handles special argument expansion

### Architecture Awareness

* CLI framework: `clap` 4 with derive macros; subcommands: `schema`, `index`, `process`, `append`, `stats`, `install`
* Entry point: `main.rs` → `lib.rs::run()` → `Commands` enum dispatch
* Type system: `schema::ColumnType` ↔ `data::Value` (mirrored enums)
* Entirely synchronous — no async runtime

---

Proceed with the user's request by executing the Required Steps in order for the specified `spec-name` and `phase-number`.
