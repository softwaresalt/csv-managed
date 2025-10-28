# Schema Inference Internals

This document provides a deep dive into how `csv-managed` infers column datatypes, handles placeholder ("NA") tokens, promotes Currency, derives fixed decimal precision/scale, and applies overrides during the `schema probe` / `schema infer` workflow. It supplements usage-oriented examples in `schema-examples.md` and the CLI flag reference in `cli-help.md`.

## Supported Datatypes

| Datatype | Description | Key Parsing Notes |
|----------|-------------|-------------------|
| String   | Fallback / unclassified values | Any non-empty token that fails all other parsers or is mixed with incompatible forms. |
| Integer  | 64-bit signed whole number | Rejects multi-digit leading zeros (except single `0`); parentheses for negative not used (those forms become Currency/Decimal candidates). |
| Float    | IEEE f64 approximation | Appears when numeric tokens overflow decimal precision or scale limits, or mixed integer/decimal without a stable fixed scale. Scientific notation accepted (e.g. `1.23e5`). |
| Decimal(p,s) | Fixed precision/scale numeric | Precision ≤ 28; tokens with consistent scale promote to Decimal when no overflow occurs. Parentheses negative forms accepted. |
| Currency | Monetary value enforcing scale 0, 2, or 4 | Symbols `$ € £ ¥` or parentheses negatives allowed; thousands separators (`,`), spaces ignored; promoted early only under strict conditions (see algorithm). |
| Boolean  | Canonical or flexible tokens | Accepted tokens (case-insensitive): `true false t f yes no y n 1 0`. Mixed forms still parse as Boolean. |
| Date     | Calendar date | Attempts multiple common formats: `YYYY-MM-DD`, `DD/MM/YYYY`, `MM/DD/YYYY`, and dot-separated variants; ambiguous day/month ordering resolves via format matching. |
| DateTime | Date plus time (seconds optional) | Accepted fallback patterns: `%Y-%m-%d %H:%M:%S`, `%Y-%m-%dT%H:%M:%S`, `%d/%m/%Y %H:%M:%S`, `%m/%d/%Y %H:%M:%S`, `%Y-%m-%d %H:%M`, `%Y-%m-%dT%H:%M`. Milliseconds / timezone offsets require explicit mapping. |
| Time     | Time of day | `HH:MM[:SS][.fraction]`; fractional seconds appear only in format hints—no fixed scale enforcement. |
| Guid     | UUID/GUID | Hyphenated or 32 hex characters (with or without braces). Case-insensitive. |

## Header Presence Detection

Before datatype voting begins the engine performs a lightweight header detection pass (`detect_csv_layout`):

* Reads up to the first 6 physical rows (constant: `HEADER_DETECTION_SAMPLE_ROWS`).
* Classifies each token in the first row as:
  * header‑like: non-empty, not data-like, and either contains alphabetic characters or matches a curated dictionary (`id`, `date`, `amount`, `status`, etc.).
  * data‑like: parses as boolean, integer/decimal/float/currency, date, datetime, time, or guid.
* Per column, if subsequent sampled rows contain any data‑like token where the first-row token is header‑like, this yields a header signal; if the first-row token is data‑like it yields a data signal.
* Early exits: more data‑like than header‑like tokens in row 1 -> treat as headerless; empty/whitespace only row 1 -> headerless.
* Tie resolution hierarchy:
  1. Compare accumulated header vs data signals.
  2. If tied, require at least one dictionary hit plus ≥1 header‑like token.
  3. Otherwise fall back to comparing counts of header‑like vs data‑like tokens.
* When headerless, synthetic zero-based names `field_0..field_{N-1}` are generated; the schema records `has_headers: false` so downstream readers treat the first physical row as data.

Override / correction strategy:

* When running `schema probe` / `schema infer`, supply `--assume-header <true|false>` to bypass the heuristic for that run (the resulting schema captures the choice in `has_headers`).
* Edit the persisted schema and set `has_headers: true` or `false` manually.
* Rename synthetic fields to semantic names after inference (the stored `has_headers` flag prevents reinterpreting row 1 on subsequent runs).
* For borderline cases (e.g., all alphabetic but genuinely data) force headerless by setting `has_headers: false` and renaming columns explicitly.

Limitations & rationale:

* The heuristic intentionally favors precision over recall—false positives (declaring header where none exists) are mitigated by requiring multiple signals or dictionary reinforcement.
* Heuristic does not currently use uniqueness scoring; future enhancements may incorporate duplicate token analysis and frequency divergences.
* Streaming cost is negligible (≤6 rows decoded once) and amortized across full inference / processing.

## Placeholder Tokens

The inference engine ignores tokens considered *placeholders* for voting purposes. They are tracked separately and later converted into `replace` entries if requested. A token is treated as a placeholder if, after trimming and lowercasing (and stripping a leading `#`), it matches any of:

```text
na, n/a, n.a., null, none, unknown, missing, invalid*, (any sequence of only '-')
```

Notes:

* `invalid*` means any token starting with `invalid` (e.g. `invalid_value`, `invalid-123`).
* Leading `#` (e.g. `#N/A`) is stripped before matching.
* Placeholder tokens do not increment non-empty vote counters and cannot cause a fallback to String unless *other* unclassified tokens are present.

Policies (CLI flags):

* `--na-behavior empty` (default): captured placeholder tokens become replacement array entries mapping each token to an empty string when the schema is written (preview/diff/write). Probe-only mode displays suggestions.
* `--na-behavior fill --na-fill NULL`: tokens map to a fill value (`NULL` here). Omitting `--na-fill` defaults to an empty string.

Internals: `PlaceholderSummary` collects counts per column; `apply_placeholder_replacements` injects `replace: - { value, replacement }` entries during infer preview/write/diff.

## Sampling

* `--sample-rows N` controls the maximum number of data rows consumed for inference. `N=0` means a full scan.
* If fewer than `N` rows exist, all are used; the footer clarifies actual vs requested.
* `decode_errors` counts any character decoding failures (e.g., mismatched encoding) per cell; such cells are skipped.
* Empty cells and whitespace-only cells are ignored (do not contribute votes).

## Numeric Token Analysis

Parsing pipeline for numeric candidates uses a single pass (`analyze_numeric_token`):

1. Detect sign, parentheses negative, currency symbols, thousands separators, underscores, scientific notation markers (`e`/`E`).
1. Normalize mantissa + exponent into an integer digit sequence + scale.
1. Compute: `integer_digits`, `scale`, `precision = integer_digits + scale` (with guard for all-zero scale-only values).
1. Classify tokens:

* Integer: no decimal point/exponent and within precision limit.
* Decimal: presence of decimal point or exponent after normalization and precision/scale ≤ limits.
* Float: overflow beyond max precision/scale OR forms not satisfying integer/decimal constraints.

1. Flag currency suitability when scale is 0, 2, or 4.

Edge Cases:

* Multi-digit leading zeros (e.g., `0012`) reject Integer classification to avoid accidental zero-padded IDs; such tokens may still become Decimal/Float.
* Exponent forms with large expansion `1e30` degrade to Float when precision exceeds limits.
* Parentheses denote negatives (accounting style) and are treated similarly to a leading minus sign for classification.

### Decimal Specification Assembly

After processing all tokens in a column:

* `decimal_matches > 0` makes the column eligible for Decimal.
* Aggregate maxima: `decimal_max_scale`, `decimal_max_integer_digits`, `decimal_max_precision`.
* Combine integer digits across plain integer tokens and decimal tokens to ensure enough precision.
* If computed precision or scale exceed hard limits (precision/scale > 28) OR any overflow flagged, downgrade to Float.
* Resulting spec written as `decimal(precision,scale)`; canonical formatting retained when serializing YAML.

## Currency Promotion Logic

Two promotion paths in `TypeCandidate::decide()`:

1. Early Promotion: If every non-empty token is currency-compliant (allowed scale) AND the ratio of symbol-bearing tokens meets threshold (≥30%), the datatype becomes `Currency` immediately—before considering Decimal or Float.
2. Majority Fallback: Later in decision order, if Currency holds a majority (>50% of non-empty tokens) and at least one symbol has appeared, it is selected.

Symbol Ratio Calculation:

```text
currency_symbol_hits * 100 >= non_empty * 30
```

Allowed scales: 0, 2, 4. If a mix includes scale 3 or >4 without normalization, currency majority fails and numeric types compete as usual.

## Decision Order

`TypeCandidate::decide()` (simplified pseudocode):

```text
if non_empty == 0: String
if unclassified > 0: String
if majority(boolean): Boolean
if promote_currency(): Currency   # 100% compliant, ≥30% symbols
if decimal_spec(): Decimal(p,s)   # valid spec synthesized
if decimal_matches > 0: Float     # inconsistent or overflow
if majority(integer): Integer
if majority(currency) and currency_symbol_hits > 0: Currency
if majority(float): Float
if majority(date): Date
if majority(datetime): DateTime
if majority(time): Time
if majority(guid): Guid
else: String
```

Notes:

* A single unclassified token (i.e., fails every parser) forces `String` early—even if many numeric/date tokens parse successfully. This safeguards against dirty mixed-format columns being mis-typed.
* Decimal does **not** require strict majority—any valid spec with zero overflow promotes before Integer/Float majority checks. This favors preserving precision.
* Date precedes DateTime in majority evaluation, meaning a tie (no majority) that still has only parseable date/datetime tokens but includes unclassified values will drop to String rather than choose Date arbitrarily.

## Overrides and Renames

* Overrides (`--override col:Type`) apply after voting; the probe table marks affected rows in the `override` column with `type`.
* `--mapping` applies snake_case renames with collision prevention. Renames do not influence datatype inference, and the emitted mapping table still carries a `suggested` column with the auto-generated snake_case alias for each header. The probe table's `rename` column appends `(suggested)` to highlight aliases that were auto-generated by the command. Preview runs (`schema infer --preview --mapping`) emit the YAML first, followed by the mapping table.
* The output alias is used by downstream commands (`process`, `stats`, expressions) but the schema still stores the original header.

## Placeholder Replacement Injection

During `schema infer` (including `--preview` and `--diff`):

1. Capture placeholder tokens while scanning.
2. Apply NA policy: empty or fill with token.
3. Append each token to the column's `replace` array unless already present.
4. `--replace-template` ensures every column has an empty `replace: []` scaffold when requested.

Probe-only (`schema probe`) renders a "Placeholder Suggestions" section but does not modify files.

## Schema Version and Signature

* `schema_version` auto-injected when absent (currently `1.1.0`).
* Signature displayed in probe/infer output (`Header+Type Hash`) is a SHA-256 over ordered `name:datatype` pairs; snapshots lock this signature.
* Changes to column order, header text, or inferred datatype alter the signature even if displayed samples remain similar.

## Format Hints

`format_hint_for()` supplies guidance for Date/DateTime/Time, Boolean token styles, numeric categories (thousands separators, currency symbols), integer leading zeros, and Decimal specs (precision/scale). They appear per column in probe/infer tables to help decide whether overrides or mappings are necessary.

## Observations Column

Probe and infer output now surface sampling diagnostics inline via the `observations` column. Each row notes non-empty vs. empty counts, representative sample values, placeholder token sightings, and overflow buckets, eliminating the need to scan separate summary sections while retaining the same level of detail.

## Troubleshooting Guide

| Symptom | Likely Cause | Resolution |
|---------|--------------|-----------|
| Column inferred as String despite mostly numbers | Presence of at least one unclassified token; or invalid numeric formats (leading zeros, stray suffix) | Clean the offending token(s) or add replacements; use `--override` if semantically numeric. |
| Float inferred instead of Decimal | Mixed scales or precision overflow (>28 digits) | Introduce rounding/truncation via `datatype_mappings` or ensure consistent scale; override only if acceptable data loss. |
| Currency not detected | <30% symbol-bearing or mixed unsupported scales (e.g. 3 decimals) | Add currency symbols to a representative subset or normalize scale via preprocessing. |
| DateTime inferred where Date expected | Majority (or tie) of tokens include time component and no unclassified values | Override to `Date` or supply mapping chain DateTime→Date if time is extraneous. |
| Boolean inferred as String | Inconsistent tokens (`True`, `0`, `maybe`) causing unclassified voting | Add replacements (map `maybe` to `unknown`) then re-probe; or force override. |
| Snapshot mismatch in CI | Layout or inference change after code refactor | Review diff; accept change by updating snapshot or investigate unintended heuristic drift. |
| Placeholder replacements missing after infer | Ran `schema probe` instead of `schema infer`; or omitted NA flags | Use `schema infer --preview --na-behavior fill --na-fill NULL` to verify injection before writing. |
| Guid misclassified as String | Contains surrounding text or malformed section | Clean values; ensure consistent hyphenated or raw 32-hex format. |
| Header row treated as data (headerless inference) | First row predominantly numeric/boolean/date-like tokens; insufficient alphabetic/dictionary signals; subsequent rows resemble first row structurally | Re-run `schema probe`/`schema infer` with `--assume-header true` or edit the persisted schema, set `has_headers: true`, and rename columns. Optionally insert a dummy first line with clear header tokens before inference, then remove after the schema is saved. |
| Data row mistaken for header | First row has many alphabetic tokens or matches common header dictionary; later rows sparse or similar; short sample (≤6 rows) produces balanced signals | Re-run with `--assume-header false` or set `has_headers: false` in the schema and regenerate column names (or keep synthetic). If already inferred, just toggle the flag—no need to re-run inference unless you want different names. Consider adding one more representative data row near top before inferring. |
| Mixed header/data confusion across files | Some files in a batch have headers, others do not; heuristic differs per file | Standardize upstream (add or remove headers), re-run per subset with `--assume-header true/false`, or create two schemas (`has_headers true/false`) and process separately. Avoid mixing in a single append/index operation. |

## Best Practices Summary

1. Use `schema probe --sample-rows 250` initially for a representative sample; escalate to full scan (`0`) only if needed.
2. Normalize placeholders with `--na-behavior fill --na-fill NULL` early to avoid dilution when later introducing mappings.
3. Lock inference with a snapshot once satisfied: `schema infer ... --snapshot tmp/orders.snap`.
4. Prefer Decimal over Float for financial/precision-sensitive columns—allow engine to promote automatically or override intentionally.
5. Keep currency symbols on ≥1/3 of sample rows for auto-promotion; otherwise declare type manually.
6. Use overrides sparingly—clean data and let deterministic heuristics work unless a domain rule mandates a type.
7. Add `--replace-template` during initial schema creation to scaffold replacement arrays for iterative normalization.
8. Re-run `schema infer --diff existing-schema.yml` after data format changes to audit drift before committing schema edits.

## Minimal Examples

### 1. Probe with NA Empty Policy

```powershell
./target/release/csv-managed.exe schema probe -i tests/data/placeholders.csv --na-behavior empty --sample-rows 0
```

### 2. Infer Full Scan With Fill Policy and Template

```powershell
./target/release/csv-managed.exe schema infer -i tests/data/big_5_players_stats_2023_2024.csv \
  --sample-rows 0 --mapping --replace-template --na-behavior fill --na-fill NULL \
  -o tmp/big5_schema.yml
```

### 3. Dry Run Diff Against Existing Schema

```powershell
./target/release/csv-managed.exe schema infer -i data/orders.csv --diff data/orders-schema.yml --preview --sample-rows 0
```

### 4. Snapshot Guard

```powershell
./target/release/csv-managed.exe schema probe -i data/orders.csv --sample-rows 50 --snapshot tmp/orders_probe.snap
```

### 5. Override Edge Column

```powershell
./target/release/csv-managed.exe schema infer -i data/ids.csv --override id:Integer --sample-rows 0 -o tmp/ids-schema.yml
```

## Pseudocode (Condensed)

```text
layout = detect_csv_layout(path)  # header detection & synthetic names if needed
headers = layout.headers (first row or generated field_0..field_N)
for each header:
  init TypeCandidate / sample accumulators
for each data row (respecting layout.has_headers):
  decode cells
  stop if sample limit reached
  for each cell:
    skip empty
    if placeholder -> record summary; continue
    candidate.update(cell)
decide datatype for each column via TypeCandidate::decide()
apply overrides
apply snake_case mapping if requested
inject placeholder replacements (infer only)
render report (probe/infer) with samples, format hints, hash, summaries, placeholder suggestions
persist schema (infer only unless --preview)
```

## Future Enhancements (Planned)

* Composite primary key suggestion and hashing index.
* Extended temporal format inference (timezone offsets, fractional seconds) with confidence scoring.
* Dynamic sampling strategy (early convergence detection for very large files).
* Incremental schema refinement mode (merge new sample observations without full re-scan).

---
For additional usage patterns and majority voting examples, see `schema-examples.md`. For CLI flag syntax, refer to `cli-help.md`. Report inaccuracies or request clarifications via the issue tracker.
