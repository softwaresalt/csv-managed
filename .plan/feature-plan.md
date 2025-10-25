# Version 1.1.0

Starting with a plan to switch schema files from JSON to YAML:

## Plan Overview

Outline goals, compatibility requirements, and success criteria.
Inventory current JSON touchpoints (schema read/write, tests, fixtures, docs).
Design YAML handling: parsing, serialization, CLI behaviors, schema version strategy.

## Detailed Steps

1. Requirements & Compatibility
	- No backward compatibility expectations (e.g., do not continue accepting .schema JSON. Migrate to .yml).
	- Success metrics: performance parity, error messaging, doc updates.

2. Code Inventory
	- Catalog modules that read/write schema JSON (schema.rs, schema_cmd.rs, process.rs, etc.).
	- Identify tests/fixtures referencing JSON (integration tests in tests, sample schemas in data, documentation snippets).

3. YAML Strategy
	- Choose serialization crate (likely serde_yaml) and assess feature parity with serde_json.
	- Change the naming convention for schema files to "-schema.yml"
	- Map schema structs to ensure round-trip compatibility; plan migration path for schema_version.
  
4. Dual Support & Migration
	- No dual support needed: migrate only.

5. Implementation Tasks (post-approval)
	- Update schema load/save functions to abstract format detection.
	- Add CLI options for preferred output format.
	- Regenerate fixtures/docs in YAML.
	- Update tests to cover both formats; ensure golden files updated.
	- Provide migration utility/instructions.

6. Documentation & Release Notes
	- Update README, schema-examples.md, CLI help, examples, examples.bat.
	- Document migration steps in CHANGELOG/release notes.

7. Validation Strategy
	- Define test matrix (unit, integration, CLI spot checks).
	- Plan regression runs (cargo test, targeted examples).
	- Outline manual QA (verify schema verify/process with YAML).

8. Risks & Mitigations
	- Parsing differences (comments, anchors) → restrict features or sanitize.
	- Performance impact → benchmark large schemas.
	- No need to provide a schema convert command option.
