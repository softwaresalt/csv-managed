<!--
Sync Impact Report:
- Version change: none → 1.0.0
- List of modified principles:
  - Added: Streaming / Iterators
  - Added: Separation of Concerns
  - Added: Zero-Copy / Borrowing
  - Added: Explicit Error Types
  - Added: Deterministic Performance
  - Added: Extensibility via Traits
  - Added: Config-First
- Added sections:
  - Rust Coding Standards
  - Testing Strategy
- Removed sections: None
- Templates requiring updates:
  - ✅ .specify/templates/plan-template.md (No changes needed)
  - ✅ .specify/templates/spec-template.md (No changes needed)
  - ✅ .specify/templates/tasks-template.md (No changes needed)
- Follow-up TODOs: None
-->
# csv-managed Constitution

## Core Principles

### I. Streaming / Iterators

To handle very large datasets with minimal memory footprint, operations MUST be implemented using streaming iterators. Avoid loading entire files or large datasets into memory. Favor `csv::Reader` with `byte_records()` or `records()` and wrap them in lazy adapters.

### II. Separation of Concerns

To simplify maintenance and improve modularity, the codebase MUST be organized into distinct modules with clear responsibilities. Key modules include: parsing, schema, indexing, stats, filtering, expressions, and CLI.

### III. Zero-Copy / Borrowing

To reduce memory allocations and improve performance, code MUST prefer borrowed types (`&str`, `&[u8]`) over owned types (`String`) where possible. Use `Cow<'_, str>` for conditional ownership. Avoid unnecessary cloning.

### IV. Explicit Error Types

To improve debuggability and provide clear error messages, operations that can fail MUST return a `Result<T, E>`. Use custom error enums with `thiserror` for distinct error conditions, and propagate errors using the `?` operator. Do not discard errors silently.

### V. Deterministic Performance

To ensure reproducible and predictable runs, operations MUST avoid hidden global state. Costly features should be gated behind explicit command-line flags.

### VI. Extensibility via Traits

To support future column operations and transformations, core logic SHOULD be abstracted using traits. This allows for adding new functionality without modifying core modules.

### VII. Config-First

To support batch pipelines and repeatable runs, the tool MUST treat configuration files (YAML for schema, JSON for pipelines) as canonical inputs.

## Rust Coding Standards

- **Formatting**: `rustfmt` MUST be used to enforce a consistent style.
- **Linting**: `cargo clippy --all-targets --all-features -D warnings` MUST pass before merging. No warnings are permitted.
- **Error Handling**: Never silently discard errors. Propagate with `?`. Use `Result<T, E>` for fallible operations.
- **Unsafe Code**: Avoid `unsafe` code. If it is absolutely necessary, it MUST be isolated in a dedicated module with extensive comments explaining its invariants, preconditions, and how Undefined Behavior is avoided.
- **Documentation**: All public items (functions, structs, enums, traits, modules) MUST be documented with Rustdoc, including examples, complexity, and error cases.

## Testing Strategy

- **Unit Tests**: Located inline in `src/` modules to validate pure functions and small logic units.
- **Integration Tests**: Located in `tests/*.rs` to validate cross-module behavior and CLI workflows.
- **Property Tests**: Used with the `proptest` feature to fuzz parsers and schema inference.
- **Snapshot Tests**: Used with `insta` to ensure stable textual outputs for commands like `schema list` and `stats`.
- **Test Coverage**: Every public parser or transformer MUST have tests for both the success path and at least one failure path.

## Governance

This constitution supersedes all other practices and guidelines. All contributions, whether from humans or AI, MUST adhere to these principles.

- **Compliance**: All pull requests and reviews MUST verify compliance with this constitution. Any deviation requires explicit justification and approval.
- **Amendments**: Amendments to this constitution require a pull request, documentation of the change, and an approved migration plan if the change is breaking.
- **Guidance**: For runtime development guidance, refer to the `.github/copilot-instructions.md` file.

**Version**: 1.0.0 | **Ratified**: 2025-11-06 | **Last Amended**: 2025-11-06
