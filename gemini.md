# Gemini Agent Guidance for the `csv-managed` Rust Project

This document provides guidance for Gemini Code Assist when working on the `csv-managed` project. Please adhere to these instructions to ensure consistency, quality, and alignment with the project's goals. This is a Rust project.

## 1. Project Overview

`csv-managed` is a Rust crate designed for high-performance, low-allocation reading and writing of CSV (Comma-Separated Values) files. The primary goal is to provide a robust, flexible, and idiomatic API for developers working with CSV data in Rust applications.

## 2. Core Principles

- **Performance:** Prioritize high-throughput and low-memory allocation. Leverage Rust's zero-cost abstractions, ownership model, and concurrency features. Changes should be benchmarked using tools like `criterion`.
- **Robustness:** Handle malformed CSV data gracefully. Provide clear error messages and configurable parsing options.
- **Clarity & Readability:** Code should be idiomatic, easy to understand, and maintain. Follow standard Rust conventions as enforced by `rust fmt` and `clippy`.
- **Testability:** All new features and bug fixes must be accompanied by comprehensive unit and integration tests. Maintain high code coverage.

## 3. Development Workflow

1. **Understand the Goal:** Carefully analyze the user's request to fully understand the desired outcome. If the request is ambiguous, ask clarifying questions.
2. **Analyze Existing Code:** Before writing new code, thoroughly examine the existing codebase to understand its structure, patterns, and conventions. Identify the best places to introduce changes.
3. **Propose Changes:** Provide changes in the form of `diff` blocks for existing files. For new files, provide the full content.
4. **Write/Update Tests:** All logical changes must be covered by tests. Add new tests for new features and update existing tests for modifications. Ensure all tests pass via `cargo test`.
5. **Maintain Code Quality:** Adhere to the coding style and conventions outlined below. Refactor where necessary to improve clarity or performance, but do so with care.
6. **Explain Your Work:** Provide a clear, concise explanation of the changes you've made and the reasoning behind them.

## 4. Rust Coding Style and Conventions

- **Edition:** Use the latest stable Rust edition (e.g., Rust 2021).
- **Formatting:** All code must be formatted with `rustfmt` using the default project configuration.
- **Linting:** All code should be free of warnings from `clippy`. Run `cargo clippy` regularly.
- **Naming:**
  - Use `PascalCase` for types (structs, enums, traits).
  - Use `snake_case` for functions, methods, variables, and modules.
    - Use `SCREAMING_SNAKE_CASE` for constants.
- **`use` statements:** Group `use` statements at the top of the module. Order them: `std`, external crates, then project modules (`crate::`, `super::`).
- **Documentation:** Add/update documentation comments (`///` for items, `//!` for modules) for all public APIs. Include examples where appropriate. This is crucial for `cargo doc`.
- **Error Handling:**
  - Use `Result<T, E>` for recoverable errors. Define custom, specific error enums for your library.
  - Use `Option<T>` for values that might be absent.
  - Use `panic!` only for unrecoverable errors that indicate a bug (e.g., a violated invariant). Avoid panicking in library code that the user can trigger.
  - Leverage the `?` operator for concise error propagation.

## 5. Testing Strategy

- **Framework:** Use the built-in test framework (`#[test]`). For benchmarking, use `criterion`.
- **Assertions:** Use the standard assertion macros: `assert!`, `assert_eq!`, and `assert_ne!`. For more complex assertions, a crate like `assert_matches` may be used if it is a project dependency.
- **Test Structure:** Follow the **Arrange-Act-Assert** pattern to structure your tests clearly.
  - **Arrange:** Set up the test objects and preconditions.
  - **Act:** Execute the method being tested.
  - **Assert:** Verify the outcome is as expected.
- **Coverage:** Aim for comprehensive test coverage. Use tools like `cargo-tarpaulin` to measure it. Test edge cases, invalid inputs, and expected failure modes (`#[should_panic]`) in addition to the "happy path".
- **Test Location:**
  - Place unit tests in a `#[cfg(test)] mod tests { ... }` block at the bottom of the file they are testing.
  - Place integration tests in the `tests/` directory at the crate root.

## 6. Project-Specific Guidance

- **Core Abstractions:** Be familiar with the main modules (`index`, `schema`, `process`, etc.) and their primary data structures, such as `index::CsvIndex` and `schema::Schema`. Changes should respect the existing modular architecture centered around CLI commands.
- **Immutability:** Prefer immutability. Use `let` by default and `mut` only when necessary. This is a core Rust principle.
- **Ownership and Borrowing:** Write code that is mindful of ownership. Prefer passing references (`&T` or `&mut T`) over owned values (`T`) to avoid unnecessary clones and allocations.
- **Performance-Critical Code:** When working in performance-sensitive areas (e.g., the core parsing loop), be mindful of every allocation. Prefer string slices (`&str`) over owned `String`s. Use iterators effectively, as they are zero-cost abstractions.

By following these guidelines, you will help maintain and improve the quality of the `csv-managed` crate.

<!-- BACKLOG.MD MCP GUIDELINES START -->

<CRITICAL_INSTRUCTION>

## BACKLOG WORKFLOW INSTRUCTIONS

This project uses Backlog.md MCP for all task and project management activities.

### CRITICAL GUIDANCE

- If your client supports MCP resources, read `backlog://workflow/overview` to understand when and how to use Backlog for this project.
- If your client only supports tools or the above request fails, call `backlog.get_workflow_overview()` tool to load the tool-oriented overview (it lists the matching guide tools).

- **First time working here?** Read the overview resource IMMEDIATELY to learn the workflow
- **Already familiar?** You should have the overview cached ("## Backlog.md Overview (MCP)")
- **When to read it**: BEFORE creating tasks, or when you're unsure whether to track work

These guides cover:

- Decision framework for when to create tasks
- Search-first workflow to avoid duplicates
- Links to detailed guides for task creation, execution, and completion
- MCP tools reference

You MUST read the overview resource to understand the complete workflow. The information is NOT summarized here.

</CRITICAL_INSTRUCTION>

<!-- BACKLOG.MD MCP GUIDELINES END -->
