---
title: "ADR-002: Code Coverage Tool Selection"
status: Proposed
date: 2023-10-27
---

# ADR-002: Code Coverage Tool Selection

**Status:** Proposed
**Date:** 2023-10-27

## Context

To improve code quality and ensure test effectiveness, the project needs a standardized tool for measuring code coverage. The v1.1.0 feature plan (F3) calls for building a test harness that integrates coverage reporting into the CI/CD pipeline. This will allow us to track test coverage over time, identify untested code, and enforce quality gates on pull requests.

## Decision Drivers

The selection of a coverage tool will be based on the following criteria:

- **Accuracy:** The tool must accurately report line and branch coverage for modern Rust code, including generics, macros, and integration tests.
- **Ease of Use:** The tool should be simple to install and execute both for local development and within the CI environment.
- **CI Integration:** It must produce standard output formats (e.g., `lcov`) that are compatible with popular coverage reporting services like Codecov or Coveralls.
- **Platform Support:** It must work reliably across all target platforms (Linux, macOS, and Windows).
- **Toolchain Stability:** The solution should not require the use of the nightly Rust toolchain or unstable compiler features, ensuring a stable build process.

## Considered Options

1. `cargo-llvm-cov`
2. `cargo-tarpaulin`
3. Manual setup with `grcov`

### Option 1: `cargo-llvm-cov`

This tool leverages LLVM's source-based code coverage instrumentation, which is built into the Rust compiler toolchain.

- **Pros:**
  - **High Accuracy:** Generally considered the most accurate coverage tool for Rust, as it uses the compiler's own instrumentation capabilities. It correctly handles complex Rust features like macros and generics.
  - **Stable Toolchain:** Works on the stable Rust toolchain. It requires the `llvm-tools-preview` component but does not force the entire project to switch to nightly.
  - **Excellent Platform Support:** Works consistently across Linux, macOS, and Windows.
  - **Standard Output:** Natively generates `lcov` reports, making CI integration straightforward.

- **Cons:**
  - **Component Dependency:** Requires developers and the CI environment to install an extra toolchain component via `rustup component add llvm-tools-preview`.

### Option 2: `cargo-tarpaulin`

A popular coverage tool that operates by tracing ptrace system calls on Linux.

- **Pros:**
  - **Simple Setup (on Linux):** Easy to install and run with `cargo install cargo-tarpaulin`.
  - **Rich Feature Set:** Offers various configuration options and output formats.

- **Cons:**
  - **Platform Limitation:** Primarily designed for and works best on Linux. Windows and macOS support is experimental and has known limitations, making it unsuitable for our cross-platform goals.
  - **Accuracy Issues:** The ptrace-based approach can sometimes produce less accurate results for complex code or code with no system calls, and it has historically had issues with test binaries that don't terminate cleanly.

### Option 3: Manual setup with `grcov`

This approach involves manually instrumenting the code during compilation and then using the `grcov` tool to collect and format the results.

- **Pros:**
  - **Flexible:** `grcov` is a powerful tool for processing coverage data from various sources.

- **Cons:**
  - **Complex Setup:** Requires manually setting `RUSTFLAGS` and `CARGO_INCREMENTAL=0`. This process is brittle and requires nightly Rust for some instrumentation features, violating a key decision driver.
  - **High Maintenance:** The setup is more complex to maintain compared to the integrated experience offered by `cargo-llvm-cov` or `cargo-tarpaulin`.

## Decision

**Chosen Option:** `cargo-llvm-cov`

`cargo-llvm-cov` is the best choice for our project. It meets all our decision criteria, offering the highest accuracy, excellent cross-platform support, and straightforward CI integration, all while working with the stable Rust toolchain.

The minor inconvenience of installing the `llvm-tools-preview` component is a small and acceptable price for the accuracy and reliability it provides. This approach aligns with the recommendation in the v1.1.0 feature plan and provides a robust foundation for our code quality initiatives. `cargo-tarpaulin`'s platform limitations make it a non-starter, and the complexity of a manual `grcov` setup is not justified.

## Consequences

### Positive

- We will have a highly accurate and reliable method for measuring code coverage.
- The chosen tool works consistently for local development and CI across all supported platforms.
- CI integration will be simple due to native `lcov` report generation.
- The project can remain on the stable Rust toolchain.

### Negative

- A one-time setup step is required for all development and CI environments to install the `llvm-tools-preview` component. This will be documented in the project's `README.md` or `CONTRIBUTING.md`.
