---
title: "ADR-003: Schema Evolution Output Format"
status: Proposed
date: 2023-10-27
---


## Context

The v1.1.0 release introduces a schema evolution feature (F4) that compares two schemas and generates a report detailing the differences (e.g., columns added, removed, or with changed types). A key design decision is where this evolution report should be stored: as a separate file or embedded within the new schema file itself.

## Decision Drivers

- **Artifact Integrity:** The primary schema file should remain a clean, canonical definition of the *current* state, adhering to the Single Responsibility Principle.
- **Clarity of Purpose:** It should be immediately obvious to users and tools what the purpose of each file is.
- **Audit Trail:** The chosen format must support a clear and unambiguous audit trail when checked into version control.
- **Tooling Compatibility:** The format should not break existing or future external tools that parse the schema file for its primary purpose of defining data transformations.

## Considered Options

### Option 1: Separate Artifact (`<schema_name>.evo.yml`)

This approach generates the evolution report as a distinct file, separate from the schema file.

- **Pros:**
  - Keeps the primary schema file clean and focused on its single responsibility: defining the current rules.
  - Creates an explicit, point-in-time report that forms a clear audit trail in version control.
  - Avoids polluting the schema with historical metadata that is irrelevant to processing commands.
  - Ensures backward compatibility for any tools that parse the schema file.
- **Cons:**
  - Results in an additional file to manage.

### Option 2: Embedded in Schema

This approach would add an `evolution:` block directly into the generated schema file.

- **Pros:**
  - The schema and the report of its last change are contained within a single file.
- **Cons:**
  - Pollutes the schema with "write-only" historical data that is irrelevant to its core function.
  - Could lead to file bloat over time if the pattern were repeated.
  - Creates a non-standard schema format, potentially breaking external tools and requiring custom handling in our own parser.

## Decision

**Chosen Option:** Option 1 - Separate Artifact (`<schema_name>.evo.yml`)

The evolution report will be generated as a separate artifact by default. This design maintains the integrity and single responsibility of the schema file, making it easier for both humans and machines to parse. It provides a cleaner and more explicit audit trail, as the evolution report is a distinct object that can be reviewed and committed.

The minor inconvenience of managing an extra file is a worthwhile trade-off for the architectural cleanliness and clarity it provides. A future enhancement could introduce an `--embed` flag for specific use cases, but the default behavior will be to keep the artifacts separate.

## Consequences

- The schema file remains a clean, canonical definition of the current state.
- The evolution report serves as a clear, explicit artifact for auditing and version control.
- No breaking changes are introduced for tools that consume the schema file.
- Users will need to manage the separate `.evo.yml` file if they wish to retain it.
