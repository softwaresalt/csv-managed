# Datatype Mappings Deep Dive

Comprehensive guide to `datatype_mappings` chains that transform raw textual values before final type parsing.

## Purpose

Mappings provide structured, ordered conversions (e.g. String → DateTime → Date, Float rounding, String hygiene) prior to replacements and final datatype enforcement.

## Execution Order (Per Cell)

1. Raw value
2. `datatype_mappings` (in declaration order)
3. `replace` substitutions
4. Final parse into declared `datatype`

## Supported Type Transitions

| From | To | Notes |
|------|----|-------|
| String | DateTime | Optional `options.format` chrono pattern or built‑in fallbacks |
| DateTime | Date / Time | Component extraction |
| String | Date | Use explicit format or rely on inference only when stored as Date already |
| String | Integer / Float / decimal(p,s) / Currency | Numeric parsing with optional strategy |
| Float | Currency / decimal(p,s) / Integer | Enforce scale/precision or truncate |
| String | String | Hygiene (`trim`, `lowercase`, `uppercase`) |
| Float | Float | Rounding/truncation for scale stabilization |
| Integer | Float / decimal(p,s) | Upcast preservation |

Invalid or unsafe transitions abort mapping phase for that row/column (row becomes invalid in verification).

## Strategies

| Strategy | Applies To | Behavior |
|----------|-----------|----------|
| round | Float → Float, Float → Currency/Decimal, String → Float/Decimal/Currency | Midpoint away from zero; scale from `options.scale` or target spec |
| truncate | Float → Integer/Decimal/Currency; String → Decimal/Currency | Cuts extra fractional digits |
| trim | String → String | Strip leading/trailing whitespace |
| lowercase | String → String | Unicode lowercase (ASCII-fast path) |
| uppercase | String → String | Unicode uppercase |

If an unsupported strategy/type combination is declared, verification will flag invalid rows (defensive failure).

## Decimal & Currency

- Decimal spec enforced by final declared `datatype` (e.g. `decimal(18,4)`).
- Currency scale restricted to 2 or 4.
- Round vs truncate can materially change aggregation outcomes—choose deliberately.

## Example Chain (Timestamp Simplification)

```yaml
- name: ordered_raw
  rename: ordered_at
  datatype: Date
  datatype_mappings:
    - from: String
      to: DateTime
      options:
        format: "%Y-%m-%dT%H:%M:%SZ"
    - from: DateTime
      to: Date
```

## Example (Precision Control)

```yaml
- name: amount_raw
  rename: amount
  datatype: decimal(18,4)
  datatype_mappings:
    - from: String
      to: decimal(18,4)
      strategy: round
      options:
        scale: 4
```

## Mixed Hygiene & Numeric

```yaml
- name: price_dirty
  rename: price
  datatype: Currency
  datatype_mappings:
    - from: String
      to: String
      strategy: trim
    - from: String
      to: Currency
      strategy: round
      options:
        scale: 2
```

## Failure Modes & Diagnostics

| Symptom | Likely Cause | Remedy |
|---------|--------------|--------|
| All rows invalid after verify | First mapping step format mismatch | Add correct datetime `format` or sanitize upstream |
| Decimal downgraded to Float (inference) | Mixed scales before mapping | Introduce mapping for rounding/truncation to uniform scale |
| Currency parsing rejected | Scale 3 or >4, stray symbols | Clean data or apply rounding/truncation mapping |
| Strategy ignored | Unsupported pair (e.g. `lowercase` on Float) | Remove or move earlier String normalization stage |

## Design Guidelines

1. Keep chains minimal—each step adds parse overhead.
2. Prefer String hygiene before numeric parse to avoid subtle whitespace issues.
3. Use rounding (not truncate) for financial correctness unless explicit truncation rules apply.
4. Consolidate date/time normalization at ingestion; downstream stages assume canonical forms.
5. Document non-obvious mappings with comments in the schema file for team clarity.

## Decision Matrix (Override vs Mapping vs Replacement)

| Need | Mapping? | Override? | Replacement? | Note |
|------|----------|----------|--------------|------|
| Structural format change | ✔ | ✖ (unless inference failed) | ✖ | e.g. String ISO timestamp → Date |
| Force final datatype ignoring evidence | ✖ | ✔ | ✖ | Domain certainty |
| Token normalization (few discrete variants) | ✖ | ✖ | ✔ | Categorical clean-up |
| Scale rounding | ✔ | ✖ | ✖ | Numeric consistency |
| Placeholder to canonical fill | ✖ | ✖ | ✔ | Via `replace` or NA flags |

## Best Practices

- Start with inference-only schema; add mappings after confirming raw patterns.
- Use `schema verify` early after introducing new chains to catch unexpected format deviations.
- Snapshot inference output after mapping additions to detect accidental drift later.

## Future Enhancements

Planned: conditional mappings (predicate-driven), reusable mapping templates, and mapping-level metrics in verification reports.
