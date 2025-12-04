# v1.1.0 Feature Implementation Plan

## Overview

v1.1.0 focuses on structural and quality improvements that harden the codebase for future scale and feature growth. It introduces: alternative YAML backend evaluation, improved test structure and coverage harness, activation of schema evolution emission, and expanded string transformation capabilities. These items reduce technical debt, increase correctness confidence, and unlock roadmap features (schema evolution and batch/pipeline workflows).

## Goals

- Decouple YAML parsing from a single crate to reduce supply-chain and performance risk.
- Reorganize tests into `tests/` for maintainability and enable coverage reporting in CI; provide shared test utilities.
- Add a CI coverage job (initially report-only) and publish artifacts.
- Establish sustainable coverage measurement (>=80% line, >=70% branch initial targets) with automated reporting in CI.
- Implement schema evolution emission with deterministic diffs.
- Implement schema evolution emission and activate the associated test.
- Implement and emit schema evolution artifacts enabling previously ignored test to pass.
- Add basic string transforms (lowercase, uppercase, camelCase, PascalCase, snake_case and substring/regex-based) in processing and expression evaluation with clear docs and tests.
- Introduce a minimal load_fixture helper to centralizing fixture path logic.

## Features

1. YAML backend spike & ADR
2. Test refactor & shared helpers
3. Coverage harness & CI integration
4. Schema evolution emission
5. String transformation framework (initial ops)
6. Auditable Snapshot Enhancements

## Sequencing & milestones

1. Spike YAML alternatives and author ADR.
2. Move tests and add `tests/common` helpers.
3. Add coverage job to CI and validate baseline reporting.
4. Implement schema evolution emission and enable related tests.
5. Implement initial string transforms and add docs/examples.
6. Implement enhanced snapshot generation and add `--snapshot-format` option.

## Acceptance criteria

- ADR for YAML decision recorded and merged or linked.
- `cargo test` passes after reorganizing test layout.
- Coverage job is present in CI and produces artifacts.
- Evolution emitter outputs deterministic YAML diffs; related ignored test becomes active and green.
- Basic string transforms available with unit and integration tests and accompanying docs.
- Snapshots can be generated in a structured JSON format containing auditable metadata.

## Next steps

1. Review and approve with tech lead and product owner.
2. Start E1 spike branch and author ADR.
3. Implement E2: move tests and add `tests/common` helpers.

## Scope Source (Backlog Items)

1. Spike a migration to one of the alternatives for serde_yaml: `serde_yaml_ng`, `serde_yaml_ok`, `serde_yml`.
2. Refactor unit tests out of core code files into integration test files in the `tests` directory, establishing shared test helpers.
3. Build a better test harness that uses a coverage tool like `cargo llvm-cov` or `cargo tarpaulin` and integrate it into the CI pipeline.
4. Implement schema-evolution emission so the ignored test can be activated. Wire the new schema-evolution doc section into CI examples.
5. Enhance string manipulation transformations (e.g., lowercase, uppercase, snake_case, substring, regex_replace).
6. Enhance snapshot support to effectively support an auditable pipeline by adding more metadata and a structured format.

## Feature-Level Breakdown

| Feature | Description | Dependencies |
|------|-------------|--------------|
| F1 | YAML Backend Spike & Selection | None |
| F2 | Test Refactor & Layout Normalization | F1 (light) if YAML changes alter schema parsing tests |
| F3 | Coverage Harness & CI Integration | F2 (for stable layout) |
| F4 | Schema Evolution Emission Feature | F1 (parsing stability), F2 (test layout) |
| F5 | String Transformation Framework Expansion | None (parallelizable), but integrate with coverage (F3) |
| F6 | Auditable Snapshot Enhancements | None (parallelizable) |

## Architecture & Design Notes

### F1: YAML Backend

- **Abstraction:** Introduce a minimal abstraction for YAML parsing to future-proof the application. This should be a simple trait abstracting `load_from_path` and `save_to_path` operations.

  ```rust
  // In a new `src/yaml_provider.rs`
  pub trait YamlProvider {
      fn load_from_path<T: for<'de> serde::Deserialize<'de>>(&self, path: &Path) -> Result<T, anyhow::Error>;
      fn save_to_path<T: serde::Serialize>(&self, path: &Path, data: &T) -> Result<(), anyhow::Error>;
  }
  ```

- **Evaluation Criteria:** The spike should evaluate `serde_yaml_ng`, `serde_yaml_ok`, and `serde_yml` against the following criteria:
  1. **Performance:** Serialization/deserialization speed and memory usage for typical schema files (small) and large batch files (if applicable).
  2. **Error Reporting:** Quality of error messages, including line/column numbers and context. This is a known weakness of the current `serde_yaml`.
  3. **API Ergonomics:** Ease of integration with the existing `serde`-based data structures.
  4. **Maintenance & Stability:** Crate's maintenance status, release frequency, and community adoption.
  5. **Spec Compliance:** Correct handling of YAML features like anchors, aliases, and tags.

### F2: Test Refactoring

- **Structure:** Relocate unit tests from `src/` modules into corresponding files within `tests/`. For example, tests in `src/core/transforms.rs` would move to `tests/transforms_test.rs`. Use `#[cfg(test)]` modules within `tests/` files to keep helper functions private to a test module.
- **Shared Helpers:** Create a `tests/common/mod.rs` module for shared test utilities, such as:
  - Functions to create temporary CSV files and schema files.
  - Assertions for comparing command output against expected results.
  - A test context builder for setting up test environments.

### F3: Coverage Harness

- **Tool Selection:** `cargo-llvm-cov` is recommended as it often provides more accurate results for Rust code without requiring nightly toolchains or special compiler flags for the build itself.
- **CI Integration:** Add a new job to the GitHub Actions workflow (`.github/workflows/build.yml`) that runs `cargo llvm-cov --all-features --workspace --lcov --output-path lcov.info`.
- **Reporting:** Use an action like `coverallsapp/github-action` or `codecov/codecov-action` to upload the `lcov.info` artifact and report coverage on PRs.
- **Gating Strategy:** Initially, run the job in a non-blocking "report-only" mode to establish a baseline. After 2-3 PRs, introduce a failure threshold (e.g., `fail_under: 80`) to enforce coverage standards.
- **Implementation Status:** `ci.yml` now contains a `coverage` job that installs `cargo-llvm-cov`, produces `target/lcov.info`, uploads it as an artifact, and reports to Codecov with `continue-on-error: true` so we can monitor baseline numbers before enforcing thresholds.

### F4: Schema Evolution

- **Deterministic Output:** The schema evolution diff must be deterministic for reproducible tests. Sort changes first by `SchemaChangeKind` enum order, then alphabetically by column name.
- **Output Format:** The evolution report should be a separate artifact (`<schema_name>.evo.yml`) by default. This avoids polluting the canonical schema file. An `--embed-evolution` flag could be added later if needed.
- **Implementation Status:** `schema infer` and `process` now support `--evolution-base` / `--emit-evolution-base` plus optional `--evolution-output` flags. Integration tests cover CLI emission (both default `<schema>.evo.yml` naming and custom destinations), and documentation/examples show how to chain the emitted schema into downstream stages.

### F5: String Transformations

- **Implementation:** Implement transforms as functions that take a `&str` and return a `Cow<'_, str>`. This avoids allocations when the input string does not need to be modified (e.g., applying `lowercase` to an already-lowercase string).
- **Integration:** These functions should be integrated into the expression engine (evalexpr) as custom functions. This allows them to be used in `--filter` and `--project` expressions.

  ```rust
  // Example registration
  let mut context = evalexpr::HashMapContext::new();
  context.set_function("lowercase", evalexpr::Function::new(|arg| {
      // ... implementation ...
  })).unwrap();
  ```

- **Scope:** The initial implementation should include: `lowercase`, `uppercase`, `snake_case`, `trim`, `substring(start, length)`, and `regex_replace(pattern, replacement)`. `PascalCase` and `camelCase` can be deferred if they prove complex.

### F6: Auditable Snapshot Enhancements

- **Goal:** To make snapshots more robust for auditing by embedding critical metadata and providing a machine-readable format.
- **Structured Format:** Introduce a new --snapshot-format {format} option to schema infer.

  - text (default): The existing human-readable format for backward compatibility.
  - json: A new structured JSON format.
    - **Enhanced Metadata (for JSON format):** The JSON snapshot will contain a metadata object with the following fields:
  - timestamp: ISO 8601 timestamp of when the snapshot was generated.
  - version: The version of csv-managed that created the snapshot.
  - source_file: The path to the input file (-i).
  - command: The full command-line arguments used to generate the snapshot.
  - notes: An optional user-provided string via a new --snapshot-notes "message" argument.
    - **Core Snapshot Data:** The JSON object will also contain the existing snapshot information in a structured way:
  - header_hash: The SHA-256 hash of the headers and inferred types.
  -columns: An array of objects, each detailing the column name, position, and inferred_type.
  - sample_summary: The human-readable table of sample data (as a multi-line string).
    - **Implementation:**
      - Create a new serializable Snapshot struct in src/core/schema.rs.
      - Modify the schema infer command to populate this struct and serialize it based on the --snapshot-format flag.
      - The existing text format can be generated from the same Snapshot struct to ensure consistency.

### Snapshot Data Structures (Proposed)

Wrapper for deterministic output:

```rust
pub struct SchemaEvolution { pub changes: Vec, +}
// For F6: Auditable Snapshots 
#[derive(Serialize, Deserialize)]
pub struct Snapshot {
  pub metadata: SnapshotMetadata,
  pub header_hash: String,
  pub columns: Vec,
  pub sample_summary: String
}

#[derive(Serialize, Deserialize)]
pub struct SnapshotMetadata {
  pub timestamp: String, // ISO 8601
  pub version: String // Crate version
}

#[derive(Serialize, Deserialize)]
pub struct SnapshotColumn {
  pub name: String,
  pub position: usize,
  pub inferred_type: String
}
```

## Schema Change Data Structures (Proposed)

```rust
pub enum SchemaChangeKind {
  ColumnAdded,
  ColumnRemoved,
  ColumnRenamed { from: String },
  DatatypeChanged { from: DataType, to: DataType },
  // `MappingAdded` is too generic. Be more specific.
  RenameMappingAdded { from: String, to: String },
  ReplaceMappingAdded { column: String, from_value: String, to_value: String }
}

// Wrapper for deterministic output
pub struct SchemaEvolution {
    pub changes: Vec<SchemaChange>
}
```

## Outstanding Questions

1. Do we version schema files explicitly today? If not, add `version:` key increment logic? (Assumed yes for evolution; need confirmation.)
2. Should evolution output be embedded into the new schema or separate artifact (`*.evo.yml`)? (Current plan: separate unless `--embed` future flag.) --> Yes, separate.
3. Are transform operations allowed post-datatype conversion or both pre/post? (Assume post-normalization; document.) --> Transform operations should be supported both pre/post conversion.  This means that the operation should be tagged as either pre or post conversion and also that multiple transforms should be supported both pre and post conversion.
4. Should coverage thresholds differ for Windows vs Linux if tool variance emerges? (Assume unified.) --> Yes, unified.

## Assumptions

- Existing ignored test has fixture representing prior vs new schema for evolution diff.
- Expression engine can be extended with custom functions without major rewrite.
- Adding minimal abstraction for YAML will not degrade performance measurably.

-------------------------------

## Proposed `tests/common/mod.rs`

As part of the test refactor (F2), a common test helper module will be created at `tests/common/mod.rs`. Below is the proposed initial implementation, providing a `TestContext` for managing temporary files and a `CommandExt` trait for streamlined command execution and assertions.

```rust
// Allow dead code in a test utility module
#![allow(dead_code)]

use assert_cmd::Command;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use tempfile::{tempdir, TempDir};

/// Provides a temporary directory context for running tests.
///
/// The directory and its contents are automatically cleaned up when
/// `TestContext` is dropped.
pub struct TestContext {
    pub temp_dir: TempDir,
}

impl TestContext {
    /// Creates a new `TestContext` with a temporary directory.
    pub fn new() -> Self {
        TestContext {
            temp_dir: tempdir().expect("Failed to create temp dir"),
        }
    }

    /// Creates a file with the given name and content inside the temporary directory.
    ///
    /// Returns the full path to the newly created file.
    pub fn create_file(&self, name: &str, content: &str) -> PathBuf {
        let file_path = self.temp_dir.path().join(name);
        let mut file = File::create(&file_path).expect("Failed to create file");
        file.write_all(content.as_bytes())
            .expect("Failed to write to file");
        file_path
    }

    /// Returns the path to the temporary directory.
    pub fn path(&self) -> &Path {
        self.temp_dir.path()
    }
}

/// Extension trait for `assert_cmd::Command` to add common assertion patterns.
pub trait CommandExt {
    /// Asserts that the command executes successfully and its stdout contains
    /// the specified string.
    fn success_with_stdout_contains(&mut self, expected: &str);
}

impl CommandExt for Command {
    fn success_with_stdout_contains(&mut self, expected: &str) {
        let output = self.output().expect("Failed to execute command");
        assert!(output.status.success());
        let stdout = String::from_utf8(output.stdout).expect("stdout is not valid UTF-8");
        assert!(stdout.contains(expected), "Expected stdout to contain '{}', but it was:\n{}", expected, stdout);
    }
}
```

-------------------------------

## Feature Comparison with csvkit

This document outlines missing features and capabilities in `csv-managed` when compared to the popular `csvkit` suite of tools.

## Major Feature Gaps

### 1. File Conversion (`in2csv`)

- **Description:** `csvkit`'s `in2csv` tool can convert various file formats like Excel (`.xls`, `.xlsx`), JSON, and fixed-width files into CSV. `csv-managed` currently only supports CSV files as input.
- **`csvkit` tool:** `in2csv`
- **`csv-managed` equivalent:** None.
- **Priority:** High. This is a significant gap in functionality for users who work with data in multiple formats.

### 2. SQL Integration (`csvsql`)

- **Description:** `csvkit`'s `csvsql` allows users to run SQL queries directly on CSV files and can also be used to generate `CREATE TABLE` statements and load data into a SQL database.
- **`csvkit` tool:** `csvsql`
- **`csv-managed` equivalent:** None.
- **Priority:** High. This is a powerful feature for data analysis and integration with relational databases.

### 3. Data Joining (`csvjoin`)

- **Description:** `csvkit`'s `csvjoin` can perform left, right, inner, and outer joins on two CSV files based on common columns.
- **`csvkit` tool:** `csvjoin`
- **`csv-managed` equivalent:** `join` (currently commented out in the source code).
- **Priority:** Medium. The feature is planned but not yet implemented.

### 4. JSON Output (`csvjson`)

- **Description:** `csvkit`'s `csvjson` tool converts CSV files to JSON. `csv-managed` can only output snapshots to JSON, not the full data.
- **`csvkit` tool:** `csvjson`
- **`csv-managed` equivalent:** None for full data conversion.
- **Priority:** Medium. JSON is a common format for data interchange.

## Minor Feature Gaps and Enhancements

### 1. Enhanced Formatting (`csvformat`)

- **Description:** `csvkit`'s `csvformat` provides extensive options for controlling the output format, including quoting, line endings, and escape characters. `csv-managed`'s `process` command has limited options (delimiter and boolean format).
- **`csvkit` tool:** `csvformat`
- **`csv-managed` command:** `process`
- **Enhancement:** Add more formatting options to the `process` command or a new dedicated command.

### 2. Sorting Enhancements (`csvsort`)

- **Description:** `csvkit`'s `csvsort` supports case-insensitive sorting.
- **`csvkit` tool:** `csvsort`
- **`csv-managed` command:** `process --sort`
- **Enhancement:** Add a flag for case-insensitive sorting.

### 3. Stacking Enhancements (`csvstack`)

- **Description:** `csvkit`'s `csvstack` can add a grouping column to identify the origin of stacked data.
- **`csvkit` tool:** `csvstack`
- **`csv-managed` command:** `append`
- **Enhancement:** Add an option to `append` to add a grouping column.

### 4. Statistics Output (`csvstat`)

- **Description:** `csvkit`'s `csvstat` can output statistics in JSON format and allows for selecting specific statistics to display.
- **`csvkit` tool:** `csvstat`
- **`csv-managed` command:** `stats`
- **Enhancement:** Add JSON output and filtering capabilities to the `stats` command.

### 5. Column Utilities (`csvcut`)

- **Description:** `csvkit`'s `csvcut` has a utility flag (`-n`) to quickly list column names and their indices.
- **`csvkit` tool:** `csvcut`
- **`csv-managed` equivalent:** `schema --columns` (similar but requires a schema file).
- **Enhancement:** Add a more direct way to list columns from a CSV file without needing to generate a schema first, perhaps on the `schema probe` command.
