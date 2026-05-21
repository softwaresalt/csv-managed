# ADR-0002: Wire --exclude-columns Into Process Pipeline

## Status

Accepted

## Context

The `--exclude-columns` CLI flag was defined in `cli.rs` (`ProcessArgs.exclude_columns`) but was never wired into `process.rs`. The `OutputPlan::new()` method only considered `--columns` for include-based projection, leaving the exclusion path as a no-op. This gap was identified during Phase 4 (US2) validation against FR-019, which specifies both include and exclude column projection.

## Decision

Wire `--exclude-columns` into the existing `OutputPlan::new()` method by:

1. Parsing the `exclude_columns` arg the same way as `selected_columns` (split on commas, trim, collect).
2. Passing the exclusion list to `OutputPlan::new()`.
3. Building a `HashSet` from the exclusion list and skipping matching columns during the output plan construction loop.

Exclusion applies **after** include selection: if `--columns` narrows the set and `--exclude-columns` further removes from that narrowed set, both are honored.

## Consequences

- **Positive**: FR-019 is now fully implemented. Users can combine `--columns` and `--exclude-columns` for flexible projection.
- **Positive**: Zero impact on existing behavior — when `--exclude-columns` is empty (default), the `HashSet` is empty and the skip branch is never taken.
- **Negative**: None identified. The change is minimal and backwards-compatible.

## Date

2026-02-13 — Phase 4, Task T042
