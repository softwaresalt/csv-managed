# Column Naming Conventions (snake_case Preference)

Snake_case is recommended (not enforced) for portability, expression friendliness, and diff stability.

## Benefits

- Shell ergonomics: no quoting for spaces or punctuation.
- Expression reliability: simple lowercase tokens reduce parse ambiguity.
- Cross-language alignment: matches Rust, Python, JSON norms.
- Snapshot stability: avoids churn in layout hash.
- Index clarity: predictable variant specs.
- YAML friendliness: fewer quoted keys.

## Transformation Rules

1. Trim whitespace.
2. Replace punctuation / spaces with underscores.
3. Split CamelCase boundaries (`OrderDate` -> `Order_Date`).
4. Lowercase.
5. Collapse multiple underscores.
6. Remove trailing underscores.
7. Preserve meaningful digits.

## Examples

| Original | Suggested | Notes |
|----------|-----------|-------|
| `Order Date` | `order_date` | Space -> underscore |
| `OrderDate` | `order_date` | CamelCase split |
| ` Gross$ Amount (USD) ` | `gross_amount_usd` | Trim + punctuation removed |
| `Customer-ID` | `customer_id` | Hyphen -> underscore |
| `SKU#` | `sku` | Symbol removed |
| `Total.Net` | `total_net` | Period -> underscore |
| `ShipTime(s)` | `ship_time_s` | Parentheses removed; unit kept |

## Collision Handling

Append domain or ordinal: `amount` + `amount_original`, `status` + `status_raw`.

## Aliases

Using `--mapping` emits a table with a `suggested` column. Adopt suggestions by editing `rename:` in the schema.

## When Not To Change

Legacy system or contractual names (`CustomerID`, `MFRPartNo`) can remain; retain raw name and add snake_case alias for internal processing.

## Quick Heuristic

If a header would need quotes in a shell, convert it.

## Automation Idea

Future improvement: optional `--auto-snake` flag to persist suggested aliases directly.
