# ADR 0001: CSV Output Quoting Strategy

**Status**: Accepted
**Date**: 2026-02-13
**Phase**: 001-baseline-sdd-spec / Phase 2, Task T013

## Context

FR-054 requires: "System MUST quote all fields in CSV output to ensure round-trip safety."
The plan's coding standards also state: "CSV writers use `QuoteStyle::Always` for quote safety."

The implementation in `src/io_utils.rs` used `csv::QuoteStyle::Necessary`, which only
quotes fields when they contain delimiters, quotes, or newlines. This creates a risk
that downstream consumers may misinterpret unquoted fields containing edge-case
characters, and round-trip safety is not guaranteed.

## Decision

Changed `QuoteStyle::Necessary` to `QuoteStyle::Always` in `open_csv_writer()` to
align with FR-054 and the project's coding standards.

## Consequences

### Positive

- All CSV output fields are now consistently quoted, ensuring round-trip safety
- Eliminates ambiguity for downstream parsers that may not handle unquoted edge cases
- Aligns implementation with the documented specification and coding standards

### Negative

- Output file sizes increase slightly due to additional quote characters (2 bytes per field)
- Two existing integration tests required assertion updates to account for quoted field values
  (`index_is_used_for_sorted_output`, `process_accepts_named_index_variant`)

### Risks

- Future tests that inspect raw CSV output must account for quoted fields
