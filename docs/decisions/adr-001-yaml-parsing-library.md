---
title: "ADR-001: YAML Parsing Library Selection"
status: Proposed
date: 2023-10-27
---


## Context

The project relies on YAML files for defining data schemas. The current library used for this is `serde_yaml`, which is responsible for deserializing YAML schema files into Rust structs.

The `serde_yaml` crate has several drawbacks:

1. **Unmaintained:** It has not been updated in several years, posing a security and maintenance risk (supply-chain risk).
2. **Poor Error Reporting:** When a user provides a malformed schema file, the error messages from `serde_yaml` are often generic and lack precise location information (line and column numbers), making it difficult for users to debug their files.

This ADR evaluates three potential replacements to address these issues, as outlined in the v1.1.0 feature plan (F1).

## Decision Drivers

The selection of a new YAML library will be based on the following criteria, derived from the feature plan:

- **Maintenance & Stability:** The crate must be actively maintained and considered stable for production use.
- **Error Reporting:** The library must produce high-quality, user-friendly error messages, including line and column numbers.
- **API Ergonomics:** The API should be easy to integrate with our existing `serde`-based data structures. A "drop-in" replacement is highly preferred to minimize refactoring effort.
- **Performance:** The library should have performance comparable to or better than the existing `serde_yaml` implementation.
- **Dependency Footprint:** The library should not introduce complex or problematic dependencies (e.g., requiring a C toolchain for builds).

## Considered Options

1. `serde_yaml_ng`
2. `serde_yml`
3. `serde_yaml_ok`

### Option 1: `serde_yaml_ng`

A fork of the original `serde_yaml` that aims to be a drop-in replacement with continued maintenance and improved error handling.

- **Pros:**
  - **Drop-in Replacement:** As a fork, it shares the same API as `serde_yaml`. Migration is trivial (changing the dependency name in `Cargo.toml`).
  - **Actively Maintained:** The crate is regularly updated.
  - **Pure Rust:** It builds on `yaml-rust`, a pure Rust YAML 1.2 implementation, so it adds no C dependencies.
  - **Improved Errors:** It has made specific improvements to error reporting over the original `serde_yaml`.

- **Cons:**
  - **Performance:** Performance is expected to be similar to `serde_yaml`, which may be slower than C-backed alternatives.

### Option 2: `serde_yml`

A modern `serde` wrapper for YAML, built on `libyaml-safer`, which provides safe bindings to the canonical C `libyaml` library.

- **Pros:**
  - **Excellent Performance:** `libyaml` is a highly optimized C library, and `serde_yml` is generally considered the fastest `serde`-compatible YAML crate.
  - **Good Error Reporting:** Leverages `libyaml`'s robust parser, which provides detailed error messages with location context.
  - **Actively Maintained:** The crate and its underlying dependencies are well-maintained.
- **Cons:**
  - **C Dependency:** Requires `libyaml` to be available on the build system. While the companion `-sys` crate attempts to build it from source via `cc`, this adds complexity to the build process and can be a hurdle for contributors on some platforms (especially Windows).
  - **API Differences:** The API is not a drop-in replacement for `serde_yaml`, requiring code changes beyond just updating `Cargo.toml`.

### Option 3: `serde_yaml_ok`

A newer pure-Rust YAML parser and `serde` implementation, designed from the ground up with a focus on safety and good error messages.

- **Pros:**
  - **Pure Rust:** No C dependencies, ensuring simple and portable builds with `cargo`.
  - **Excellent Error Reporting:** A primary design goal of the library is to provide high-quality, context-rich error messages.
  - **Actively Maintained:** The crate is under active development.
- **Cons:**
  - **API Differences:** The API is not a drop-in replacement for `serde_yaml`.
  - **Maturity:** As a newer library, it is less battle-tested than the alternatives which are based on older, more established parsers (`yaml-rust` or `libyaml`).
  - **Performance:** While likely sufficient for our needs (parsing small schema files), it is not as performance-focused as `serde_yml`.

## Decision

**Chosen Option:** `serde_yaml_ng`

The primary goals are to mitigate the supply-chain risk of an unmaintained dependency and to improve error reporting for users. `serde_yaml_ng` directly addresses both of these issues with the lowest possible implementation cost.

As a drop-in replacement, it requires no code changes, allowing us to realize the benefits immediately and without risk of introducing bugs during a larger refactor. The pure Rust dependency model aligns with our project's current toolchain and simplifies the build process for all contributors.

While `serde_yml` offers superior performance, our use case (parsing small schema files at application startup) is not performance-critical. The added complexity of a C dependency is not justified by the performance gain. `serde_yaml_ok` is a promising future candidate, but its relative lack of maturity and the need for a larger refactor make `serde_yaml_ng` a more pragmatic choice at this time.

## Consequences

### Positive

- The project no longer depends on the unmaintained `serde_yaml` crate.
- Users will receive better error messages when they provide invalid YAML schema files.
- The migration requires minimal effort, allowing development focus to remain on other v1.1.0 features.
- The build process remains simple and pure Rust.

### Negative

- We will not benefit from the potential performance gains of `serde_yml`. This is considered an acceptable trade-off.
- The error messages, while improved, may not be as detailed as those from a ground-up implementation like `serde_yaml_ok`. However, they are a significant improvement and sufficient for our needs.
