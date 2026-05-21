---
title: Legacy docs migration session memory
status: Final
date: 2026-05-21
branch: chore/migrate-legacy-docs
---

## Completed work

* Migrated legacy docs from `specs/`, `.plan/`, `.todos/`, `.specify/`, and `docs/adr.md` into the autoharness `docs/` structure
* Split the monolithic ADR document into standalone ADR files under `docs/decisions/`
* Updated README and repository instruction references to the migrated document paths
* Verified `cargo fmt --all -- --check`, `cargo clippy -- -D warnings`, and `cargo test`

## Files and surfaces changed

* `docs/plans/`
* `docs/decisions/`
* `docs/design-docs/`
* `docs/product-specs/`
* `docs/closure/`
* `README.md`
* `.github/copilot-instructions.md`
* `.autoharness/workspace-profile.yaml`

## Decisions and rationale

* Preserved history with `git mv` where source files still existed in legacy locations
* Kept migrated baseline SDD documents in their new homes and updated internal links instead of restoring the removed `specs/` tree
* Archived the legacy specify templates as a single reference document because the active template system now lives under `.backlogit/templates/`

## Failed approaches and anomalies

* Early reads of `specs/001-baseline-sdd-spec/*` failed because the SDD suite had already been moved on this branch before verification
* `git mv` for legacy ADR files failed because they had already been migrated; verification confirmed the target files already existed in `docs/decisions/`

## Quality gate results

* `cargo fmt --all -- --check` — passed
* `cargo clippy -- -D warnings` — passed
* `cargo test` — passed

## Next steps

* Push `chore/migrate-legacy-docs` when ready
* Open a PR if the migration needs review and merge tracking
