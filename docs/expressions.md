# Expression Examples

Extended examples for complex filtering, derivations, and analytical flag creation using `csv-managed`.

## Contents

1. Range Filters
2. Combined Filters & Expressions
3. Temporal Calculations
4. Grouping-Like Flags (Bucketing)
5. Derived Boolean Analytics Flags
6. Chaining Replacements + Derives
7. Performance Tips
8. String Case & Cleanup
9. Quoting Differences (Windows Shells)
10. Combined Example (Full)

---

## 1. Range Filters

Use two `--filter` flags to express inclusive ranges:

PowerShell (use backtick for continuation) and cmd.exe (use caret `^`). Examples below show PowerShell style.

```powershell
./target/release/csv-managed.exe process -i sales.csv -m sales-schema.yml `
  --filter "order_date >= 2024-01-01" `
  --filter "order_date <= 2024-03-31" `
  -C order_id,order_date,amount,status
```

Open range (lower bound only):

```powershell
--filter "amount >= 1000"
```

Range with `--filter-expr` (allows complex branching):

```powershell
--filter-expr 'amount >= 1000 && amount <= 5000'
```

## 2. Combined Filters & Expressions

Chain concise comparisons with one richer expression:

```powershell
./target/release/csv-managed.exe process -i sales.csv -m sales-schema.yml `
  --filter "region = US" `
  --filter "status != cancelled" `
  --filter-expr 'if(priority = "high" && amount > 750, true, amount > 1500)' `
  -C order_id,region,status,priority,amount
```

## 3. Temporal Calculations

```powershell
./target/release/csv-managed.exe process -i orders.csv -m orders-schema.yml `
  --derive 'lag_days=date_diff_days(shipped_at, ordered_at)' `
  --derive 'eta_plus2=date_add(ordered_at,2)' `
  --filter-expr 'date_diff_days(shipped_at, ordered_at) >= 1' `
  -C order_id,ordered_at,shipped_at,lag_days,eta_plus2
```

Time-of-day windowing:

```powershell
--filter-expr 'time_diff_seconds(processed_time, "06:00:00") >= 0 && time_diff_seconds(processed_time, "18:00:00") <= 0'
```

## 4. Grouping-Like Flags (Bucketing)

```powershell
--derive 'amount_bucket=if(amount<100,"small", if(amount<1000,"medium","large"))'
```

Date-based quarter flag:

```powershell
--derive 'order_quarter=if(date_diff_days(order_date,"2024-04-01")<0,"Q1", if(date_diff_days(order_date,"2024-07-01")<0,"Q2", if(date_diff_days(order_date,"2024-10-01")<0,"Q3","Q4")))'
```

## 5. Derived Boolean Analytics Flags

```powershell
--derive 'is_high_value=if(amount>1000,true,false)' \
--derive 'is_domestic=if(country="US",true,false)' \
--derive 'needs_review=if(is_high_value && status!="shipped",true,false)'
```

## 6. Chaining Replacements + Derives

If schema defines:

```jsonc
"replace": [ "status=Pending->Open", "status=Closed (Legacy)->Closed" ]
```

Then derive on normalized values:

```powershell
--derive 'age_flag=if(status="Open" && amount>500,"GROW","STABLE")'
```

## 7. Performance Tips

| Pattern | Guidance |
|---------|----------|
| Heavy numeric derives | Avoid storing large intermediate vectors; keep arithmetic minimal. |
| Complex nested if() | Consider simplifying with separate derives or precomputed flags. |
| Wide column sets | Use `-C` early to narrow output and reduce expression evaluation overhead. |
| Large temporal diffs | Pre-filter rows before computing many `date_diff_days` calls. |
| Snapshot + expressions | Snapshots only cover schema inference formatting, not derive logic. |

## 8. String Case & Cleanup

Normalize identifiers or build user-facing labels with the dedicated helpers:

| Function | Example | Notes |
|----------|---------|-------|
| `camel_case(str)` | `camel_case("order status")` → `orderStatus` | Word boundaries detected across spaces, hyphens, underscores, and CamelCase transitions. |
| `pascal_case(str)` | `pascal_case("api version")` → `ApiVersion` | Same detection as `camel_case`, but capitalizes the first token. |
| `snake_case(str)` | `snake_case("HTTPStatus")` → `http_status` | Uses Unicode-aware heuristics to separate acronym runs. |
| `trim(str)` | `trim("  value  ")` → `value` | Borrowing-friendly; avoids allocation when whitespace is absent. |
| `substring(str, start, len)` | `substring(code, 0, 3)` | Operates on Unicode scalars so multi-byte characters stay intact. |
| `regex_replace(str, pattern, replacement)` | `regex_replace(id, "[^0-9]", "")` | Returns the original string when the pattern does not match. |

Combine them with derives:

```powershell
--derive 'slug=snake_case(lowercase(Product_Name))' `
--derive 'pascal_label=pascal_case(regex_replace(category,"[^A-Za-z0-9 ]",""))'
```

Or ensure display labels remain consistent before analytics:

```powershell
--derive 'camel_customer=camel_case(trim(Customer_Name))' `
--derive 'region_key=substring(snake_case(Region), 0, 12)'
```

## 9. Quoting Differences (Windows Shells)

Correct quoting avoids misinterpretation of comparison operators, inner string literals, or special characters.

| Shell | Recommended Outer Quote | Inner String Literal | Example Derived | Example Filter-Expr |
|-------|--------------------------|----------------------|-----------------|---------------------|
| PowerShell | Single quotes `'` | Double quotes `"text"` | `'channel_tag=concat(channel,"-",region)'` | `'if(amount>1000 && status="shipped", true, false)'` |
| cmd.exe | Double quotes `"` | Escaped inner quotes `\"text\"` | "channel_tag=concat(channel,\"-\",region)" | "if(amount>1000 && status=\"shipped\", true, false)" |

Guidelines:

1. Prefer single quotes around the whole expression in PowerShell; they prevent expansion of `$` variables and treat inner double quotes literally.
2. In cmd.exe you must escape inner double quotes with backslashes or duplicate quoting depending on context.
3. Avoid mixing outer double quotes and unescaped inner double quotes; it truncates the expression.
4. Time literals and date literals should remain unquoted unless they include spaces (e.g., `"2024-01-01 06:00:00"`).
5. For complex expressions, test quickly with a small `--limit` and echo the command first if unsure.

Minimal cross-shell safe pattern:

```powershell
# PowerShell
--filter-expr 'date_diff_days(shipped_at, ordered_at) >= 2 && (region = "US" || region = "CA")'

# cmd.exe
--filter-expr "date_diff_days(shipped_at, ordered_at) >= 2 && (region = \"US\" || region = \"CA\")"
```

## 10. Combined Example (Full)

```powershell
./target/release/csv-managed.exe process -i orders.csv -m orders-schema.yml `
  --filter "status != cancelled" `
  --filter "amount >= 50" `
  --filter-expr 'date_diff_days(shipped_at, ordered_at) <= 10' `
  --derive 'lag_days=date_diff_days(shipped_at, ordered_at)' `
  --derive 'is_late=if(lag_days>5,true,false)' `
  --derive 'bucket=if(amount<100,"small", if(amount<500,"mid","large"))' `
  -C order_id,ordered_at,shipped_at,lag_days,is_late,bucket,amount,status `
  --limit 100
```

---
**See also:** main README [Expression Reference](../README.md#expression-reference) for function index and pitfalls.
