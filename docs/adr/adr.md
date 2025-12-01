# Architecture Decision Record

## ADR-001: YAML Parsing Library Selection

**Status:** Accepted
**Date:** 2023-10-27

## ADR-001: Context

The project relies on YAML files for defining data schemas. The current library used for this is `serde_yaml`, which is responsible for deserializing YAML schema files into Rust structs.

The `serde_yaml` crate has several drawbacks:

1. **Unmaintained:** It has not been updated in several years, posing a security and maintenance risk (supply-chain risk).
2. **Poor Error Reporting:** When a user provides a malformed schema file, the error messages from `serde_yaml` are often generic and lack precise location information (line and column numbers), making it difficult for users to debug their files.

This ADR evaluates three potential replacements to address these issues, as outlined in the v1.1.0 feature plan (F1).

## ADR-001: Decision Drivers

The selection of a new YAML library will be based on the following criteria, derived from the feature plan:

- **Maintenance & Stability:** The crate must be actively maintained and considered stable for production use.
- **Error Reporting:** The library must produce high-quality, user-friendly error messages, including line and column numbers.
- **API Ergonomics:** The API should be easy to integrate with our existing `serde`-based data structures. A "drop-in" replacement is highly preferred to minimize refactoring effort.
- **Performance:** The library should have performance comparable to or better than the existing `serde_yaml` implementation.
- **Dependency Footprint:** The library should not introduce complex or problematic dependencies (e.g., requiring a C toolchain for builds).

## ADR-001: Considered Options

1. `serde_yaml_ng`
2. `serde_yml`
3. `serde_yaml_ok`

### ADR-001: Option 1: `serde_yaml_ng`

A fork of the original `serde_yaml` that aims to be a drop-in replacement with continued maintenance and improved error handling.

- **Pros:**
  - **Drop-in Replacement:** As a fork, it shares the same API as `serde_yaml`. Migration is trivial (changing the dependency name in `Cargo.toml`).
  - **Actively Maintained:** The crate is regularly updated.
  - **Pure Rust:** It builds on `yaml-rust`, a pure Rust YAML 1.2 implementation, so it adds no C dependencies.
  - **Improved Errors:** It has made specific improvements to error reporting over the original `serde_yaml`.

- **Cons:**
  - **Performance:** Performance is expected to be similar to `serde_yaml`, which may be slower than C-backed alternatives.

### ADR-001: Option 2: `serde_yml`

A modern `serde` wrapper for YAML, built on `libyaml-safer`, which provides safe bindings to the canonical C `libyaml` library.

- **Pros:**
  - **Excellent Performance:** `libyaml` is a highly optimized C library, and `serde_yml` is generally considered the fastest `serde`-compatible YAML crate.
  - **Good Error Reporting:** Leverages `libyaml`'s robust parser, which provides detailed error messages with location context.
  - **Actively Maintained:** The crate and its underlying dependencies are well-maintained.

- **Cons:**
  - **C Dependency:** Requires `libyaml` to be available on the build system. While the companion `-sys` crate attempts to build it from source via `cc`, this adds complexity to the build process and can be a hurdle for contributors on some platforms (especially Windows).
  - **API Differences:** The API is not a drop-in replacement for `serde_yaml`, requiring code changes beyond just updating `Cargo.toml`.

### ADR-001: Option 3: `serde_yaml_ok`

A newer pure-Rust YAML parser and `serde` implementation, designed from the ground up with a focus on safety and good error messages.

- **Pros:**
  - **Pure Rust:** No C dependencies, ensuring simple and portable builds with `cargo`.
  - **Excellent Error Reporting:** A primary design goal of the library is to provide high-quality, context-rich error messages.
  - **Actively Maintained:** The crate is under active development.

- **Cons:**
  - **API Differences:** The API is not a drop-in replacement for `serde_yaml`.
  - **Maturity:** As a newer library, it is less battle-tested than the alternatives which are based on older, more established parsers (`yaml-rust` or `libyaml`).
  - **Performance:** While likely sufficient for our needs (parsing small schema files), it is not as performance-focused as `serde_yml`.

## ADR-001: Decision

**Chosen Option:** `serde_yaml_ng`

The primary goals are to mitigate the supply-chain risk of an unmaintained dependency and to improve error reporting for users. `serde_yaml_ng` directly addresses both of these issues with the lowest possible implementation cost.

As a drop-in replacement, it requires no code changes, allowing us to realize the benefits immediately and without risk of introducing bugs during a larger refactor. The pure Rust dependency model aligns with our project's current toolchain and simplifies the build process for all contributors.

While `serde_yml` offers superior performance, our use case (parsing small schema files at application startup) is not performance-critical. The added complexity of a C dependency is not justified by the performance gain. `serde_yaml_ok` is a promising future candidate, but its relative lack of maturity and the need for a larger refactor make `serde_yaml_ng` a more pragmatic choice at this time.

## ADR-001: Consequences

### ADR-001: Positive

- The project no longer depends on the unmaintained `serde_yaml` crate.
- Users will receive better error messages when they provide invalid YAML schema files.
- The migration requires minimal effort, allowing development focus to remain on other v1.1.0 features.
- The build process remains simple and pure Rust.

### ADR-001: Negative

- We will not benefit from the potential performance gains of `serde_yml`. This is considered an acceptable trade-off.
- The error messages, while improved, may not be as detailed as those from a ground-up implementation like `serde_yaml_ok`. However, they are a significant improvement and sufficient for our needs.

-------------------------------

## ADR-002: Code Coverage Tool Selection

**Status:** Proposed

**Date:** 2023-10-27

## ADR-002: Context

To improve code quality and ensure test effectiveness, the project needs a standardized tool for measuring code coverage. The v1.1.0 feature plan (F3) calls for building a test harness that integrates coverage reporting into the CI/CD pipeline. This will allow us to track test coverage over time, identify untested code, and enforce quality gates on pull requests.

## ADR-002: Decision Drivers

The selection of a coverage tool will be based on the following criteria:

- **Accuracy:** The tool must accurately report line and branch coverage for modern Rust code, including generics, macros, and integration tests.
- **Ease of Use:** The tool should be simple to install and execute both for local development and within the CI environment.
- **CI Integration:** It must produce standard output formats (e.g., `lcov`) that are compatible with popular coverage reporting services like Codecov or Coveralls.
- **Platform Support:** It must work reliably across all target platforms (Linux, macOS, and Windows).
- **Toolchain Stability:** The solution should not require the use of the nightly Rust toolchain or unstable compiler features, ensuring a stable build process.

## ADR-002: Considered Options

1. `cargo-llvm-cov`
2. `cargo-tarpaulin`
3. Manual setup with `grcov`

### ADR-002: Option 1: `cargo-llvm-cov`

This tool leverages LLVM's source-based code coverage instrumentation, which is built into the Rust compiler toolchain.

- **Pros:**
  - **High Accuracy:** Generally considered the most accurate coverage tool for Rust, as it uses the compiler's own instrumentation capabilities. It correctly handles complex Rust features like macros and generics.
  - **Stable Toolchain:** Works on the stable Rust toolchain. It requires the `llvm-tools-preview` component but does not force the entire project to switch to nightly.
  - **Excellent Platform Support:** Works consistently across Linux, macOS, and Windows.
  - **Standard Output:** Natively generates `lcov` reports, making CI integration straightforward.

- **Cons:**
  - **Component Dependency:** Requires developers and the CI environment to install an extra toolchain component via `rustup component add llvm-tools-preview`.

### ADR-002: Option 2: `cargo-tarpaulin`

A popular coverage tool that operates by tracing ptrace system calls on Linux.

- **Pros:**
  - **Simple Setup (on Linux):** Easy to install and run with `cargo install cargo-tarpaulin`.
  - **Rich Feature Set:** Offers various configuration options and output formats.

- **Cons:**
  - **Platform Limitation:** Primarily designed for and works best on Linux. Windows and macOS support is experimental and has known limitations, making it unsuitable for our cross-platform goals.
  - **Accuracy Issues:** The ptrace-based approach can sometimes produce less accurate results for complex code or code with no system calls, and it has historically had issues with test binaries that don't terminate cleanly.

### ADR-002: Option 3: Manual setup with `grcov`

This approach involves manually instrumenting the code during compilation and then using the `grcov` tool to collect and format the results.

- **Pros:**
  - **Flexible:** `grcov` is a powerful tool for processing coverage data from various sources.

- **Cons:**
  - **Complex Setup:** Requires manually setting `RUSTFLAGS` and `CARGO_INCREMENTAL=0`. This process is brittle and requires nightly Rust for some instrumentation features, violating a key decision driver.
  - **High Maintenance:** The setup is more complex to maintain compared to the integrated experience offered by `cargo-llvm-cov` or `cargo-tarpaulin`.

## ADR-002: Decision

**Chosen Option:** `cargo-llvm-cov`

`cargo-llvm-cov` is the best choice for our project. It meets all our decision criteria, offering the highest accuracy, excellent cross-platform support, and straightforward CI integration, all while working with the stable Rust toolchain. The CI workflow now executes `cargo llvm-cov`, publishes the `lcov` artifact, and pushes results to Codecov in a non-blocking job so we can monitor coverage trends before enforcing thresholds.

The minor inconvenience of installing the `llvm-tools-preview` component is a small and acceptable price for the accuracy and reliability it provides. This approach aligns with the recommendation in the v1.1.0 feature plan and provides a robust foundation for our code quality initiatives. `cargo-tarpaulin`'s platform limitations make it a non-starter, and the complexity of a manual `grcov` setup is not justified.

## ADR-002: Consequences

### ADR-002: Positive

- We will have a highly accurate and reliable method for measuring code coverage.
- The chosen tool works consistently for local development and CI across all supported platforms.
- CI integration will be simple due to native `lcov` report generation.
- The project can remain on the stable Rust toolchain.

### ADR-002: Negative

- A one-time setup step is required for all development and CI environments to install the `llvm-tools-preview` component. This will be documented in the project's `README.md` or `CONTRIBUTING.md`.

-------------------------------

## ADR-003: Schema Evolution Output Format

**Status:** Proposed

**Date:** 2023-10-27

## ADR-003: Context

The v1.1.0 release introduces a schema evolution feature (F4) that compares two schemas and generates a report detailing the differences (e.g., columns added, removed, or with changed types). A key design decision is where this evolution report should be stored: as a separate file or embedded within the new schema file itself.

## ADR-003: Decision Drivers

- **Artifact Integrity:** The primary schema file should remain a clean, canonical definition of the *current* state, adhering to the Single Responsibility Principle.
- **Clarity of Purpose:** It should be immediately obvious to users and tools what the purpose of each file is.
- **Audit Trail:** The chosen format must support a clear and unambiguous audit trail when checked into version control.
- **Tooling Compatibility:** The format should not break existing or future external tools that parse the schema file for its primary purpose of defining data transformations.

## ADR-003: Considered Options

### ADR-003: Option 1: Separate Artifact (`<schema_name>.evo.yml`)

This approach generates the evolution report as a distinct file, separate from the schema file.

- **Pros:**
  - Keeps the primary schema file clean and focused on its single responsibility: defining the current rules.
  - Creates an explicit, point-in-time report that forms a clear audit trail in version control.
  - Avoids polluting the schema with historical metadata that is irrelevant to processing commands.
  - Ensures backward compatibility for any tools that parse the schema file.
- **Cons:**
  - Results in an additional file to manage.

### ADR-003: Option 2: Embedded in Schema

This approach would add an `evolution:` block directly into the generated schema file.

- **Pros:**
  - The schema and the report of its last change are contained within a single file.
- **Cons:**
  - Pollutes the schema with "write-only" historical data that is irrelevant to its core function.
  - Could lead to file bloat over time if the pattern were repeated.
  - Creates a non-standard schema format, potentially breaking external tools and requiring custom handling in our own parser.

## ADR-003: Decision

**Chosen Option:** Option 1 - Separate Artifact (`<schema_name>.evo.yml`)

The evolution report will be generated as a separate artifact by default. This design maintains the integrity and single responsibility of the schema file, making it easier for both humans and machines to parse. It provides a cleaner and more explicit audit trail, as the evolution report is a distinct object that can be reviewed and committed.

The minor inconvenience of managing an extra file is a worthwhile trade-off for the architectural cleanliness and clarity it provides. A future enhancement could introduce an `--embed` flag for specific use cases, but the default behavior will be to keep the artifacts separate.

## ADR-003: Consequences

- The schema file remains a clean, canonical definition of the current state.
- The evolution report serves as a clear, explicit artifact for auditing and version control.
- No breaking changes are introduced for tools that consume the schema file.
- Users will need to manage the separate `.evo.yml` file if they wish to retain it.

-------------------------------
