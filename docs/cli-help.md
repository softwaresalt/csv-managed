# CLI Help

Captured with `./target/release/csv-managed.exe <subcommand> --help` on Windows PowerShell.

For a detailed breakdown of the datatype voting algorithm, decimal and currency promotion rules, and placeholder handling internals, see `schema-inference.md` in this directory.

## Global

```text
Manage CSV files efficiently

Usage: csv-managed.exe <COMMAND>

Commands:
    schema   Create a -schema.yml file from explicit column definitions
    index    Create a B-Tree index (.idx) for one or more columns
    process  Transform a CSV file using sorting, filtering, projection, derivations, and schema-driven replacements
    append   Append multiple CSV files into a single output
    stats    Produce summary statistics for numeric columns or frequency counts via --frequency
    install  Install the csv-managed binary via cargo install
    help     Print this message or the help of the given subcommand(s)

Options:
  -h, --help     Print help
  -V, --version  Print version
```

## schema

```text
Create a -schema.yml file from explicit column definitions

Usage: csv-managed.exe schema [OPTIONS] [COMMAND]

Commands:
  probe    Display inferred schema details without writing a file
  infer    Infer schema metadata and optionally persist a -schema.yml file
  verify   Verify CSV files against a schema definition
  columns  List column names and data types from a schema file
  help     Print this message or the help of the given subcommand(s)

Options:
  -o, --output <OUTPUT>         Destination -schema.yml file path (alias --schema retained for compatibility)
    -c, --column <COLUMNS>        Column definitions using `name:type` syntax (comma-separated or repeatable)
      --replace <REPLACEMENTS>  Value replacement directives using `column=value->replacement`
  -h, --help                    Print help
```

Column definitions now accept fixed-precision decimals via `decimal(<precision>,<scale>)`. Example: `-c "amount:decimal(18,4)"` declares a column that must fit within the specified precision and scale.

Deep dive: Refer to `schema-inference.md` for inference edge cases (placeholder tokens, leading zero handling, exponent overflow to Float, currency symbol threshold, decimal spec synthesis).

### schema probe

```text
Display inferred schema details without writing a file

Usage: csv-managed.exe schema probe [OPTIONS] --input <INPUT>

Options:
  -i, --input <INPUT>
          Input CSV file to inspect
      --sample-rows <SAMPLE_ROWS>
          Number of rows to sample when inferring types (0 means full scan) [default: 2000]
      --delimiter <DELIMITER>
          CSV delimiter character (supports ',', 'tab', ';', '|')
      --input-encoding <INPUT_ENCODING>
          Character encoding of the input file (defaults to utf-8)
      --mapping
          Emit column mapping templates to stdout after probing
      --override <OVERRIDES>
          Override inferred column types using `name:type`
      --snapshot <SNAPSHOT>
          Capture or validate a snapshot with header/type hash and sampled value summaries (writes if missing)
      --na-behavior <na-behavior>
          How to treat NA-style placeholders (NA, N/A, #NA, #N/A) during inference [default: empty] [possible values: empty, fill]
      --na-fill <STRING>
          Replacement token used when --na-behavior=fill (defaults to 'null'). Applied to schema replace arrays when writing via infer.
  -h, --help
          Print help
```

When decimals appear in sampled data, the probe output lists them as `decimal(precision,scale)` along with strategy hints if mappings specify rounding or truncation.

NA Placeholder Handling: When `--na-behavior=empty` (default), tokens `NA`, `N/A`, `#NA`, `#N/A` (and legacy variants like `n.a.`) are ignored for datatype voting and surfaced in a "Placeholder Suggestions" section with proposed `replace` entries. With `--na-behavior=fill` plus optional `--na-fill` (defaults to an empty string, e.g., `--na-fill NULL`), those tokens are treated as if they held the fill value for schema replacement purposes (inferred types still ignore them). Use `schema infer` to persist the suggestions into the generated YAML.

Inference engine note: Column datatypes are selected via majority voting across successfully parsed non-empty sampled values. A value parsed as a narrower type (e.g., Integer) also counts toward broader numeric candidates (Float) until a conflicting token appears. Tie scenarios with no >50% winner fall back to the most specific type with the highest vote count; exact ties prefer simpler canonical forms (Date over DateTime) unless a temporal granularity majority emerges. Currency is promoted ahead of Float/Decimal when at least 30% of sampled values include currency symbols and all non-empty rows satisfy the currency scale rules (0, 2, or 4 decimals); otherwise the legacy majority + symbol check applies. Use `--override name:Type` for deterministic corrections and `--sample-rows 0` for full-file voting.

### schema infer

```text
Infer schema metadata and optionally persist a -schema.yml file

Usage: csv-managed.exe schema infer [OPTIONS] --input <INPUT>

Options:
  -i, --input <INPUT>
          Input CSV file to inspect
      --sample-rows <SAMPLE_ROWS>
          Number of rows to sample when inferring types (0 means full scan) [default: 2000]
      --delimiter <DELIMITER>
          CSV delimiter character (supports ',', 'tab', ';', '|')
      --input-encoding <INPUT_ENCODING>
          Character encoding of the input file (defaults to utf-8)
      --mapping
          Emit column mapping templates to stdout after probing
      --override <OVERRIDES>
          Override inferred column types using `name:type`
      --snapshot <SNAPSHOT>
          Capture or validate a snapshot with header/type hash and sampled value summaries (writes if missing)
  -o, --output <OUTPUT>
          Destination -schema.yml file path (alias --schema retained for compatibility)
      --replace-template
          Inject empty replace arrays into the generated schema as a template when inferring
      --diff <DIFF>
          Show a unified diff between an existing schema file and the inferred schema without modifying the file
      --preview
          Render the inference report and resulting schema YAML to stdout without writing a file. Suppresses --output when present.
      --na-behavior <na-behavior>
          How to treat NA-style placeholders (NA, N/A, #NA, #N/A) during inference [default: empty] [possible values: empty, fill]
      --na-fill <STRING>
          Replacement token used when --na-behavior=fill (defaults to 'null'). Added to per-column `replace` arrays for each observed NA placeholder.
  -h, --help
          Print help
```

`schema infer` writes decimal metadata into the generated YAML so downstream commands can enforce precision/scale while processing large numeric datasets. Use `--preview` to review the probe table and the exact YAML that would be written (including `--replace-template` scaffolding) without touching the filesystem, and `--diff existing-schema.yml` to inspect a unified diff against a saved schema before committing changes.

Majority voting logic identical to `schema probe`; overrides apply after voting. Currency promotion uses the same 30% symbol threshold plus full-column compliance with currency scale rules before displacing Float/Decimal. Upcoming enhancement will allow treating tokens like `NA`, `N/A`, `#NA`, `#N/A` as empty for inference to avoid diluting numeric majorities.
\
NA placeholders are already normalized: they do not count against majority votes. When `schema infer` writes a file—or when you pass `--preview` or `--diff`—observed NA tokens are injected into each affected column's `replace` array either mapping to an empty string (`--na-behavior=empty`) or to the chosen fill token (`--na-behavior=fill --na-fill <VALUE>`, defaulting to empty).

### schema verify

```text
Verify CSV files against a schema definition

Usage: csv-managed.exe schema verify [OPTIONS] --schema <SCHEMA> --input <INPUTS>

Options:
  -m, --schema <SCHEMA>
          Schema file describing the expected structure
  -i, --input <INPUTS>
          One or more CSV files to verify
      --delimiter <DELIMITER>
          CSV delimiter character
      --input-encoding <INPUT_ENCODING>
          Character encoding for input files (defaults to utf-8)
      --report-invalid [<OPTIONS>...]
          Report invalid rows by summary (default) or detail. Append ':detail' and/or ':summary' and optionally a LIMIT value
  -h, --help
          Print help
```

`schema verify` enforces decimal precision and scale exactly as defined in the schema; any value that exceeds the allowed integer digits or fractional places is reported as invalid.

### schema columns

```text
List column names and data types from a schema file

Usage: csv-managed.exe schema columns --schema <SCHEMA>

Options:
  -m, --schema <SCHEMA>
          Schema file describing the columns to list
  -h, --help
          Print help
```

## index

```text
Create a B-Tree index (.idx) for one or more columns

Usage: csv-managed.exe index [OPTIONS] --input <INPUT> --index <INDEX>

Options:
  -i, --input <INPUT>
          Input CSV file to index
  -o, --index <INDEX>
          Output index file (.idx)
  -C, --columns <COLUMNS>
          Columns to include in a single ascending index (deprecated when --spec is used)
      --spec <SPECS>
          Repeatable index specifications such as `col_a:asc,col_b:desc` or `fast=col_a:asc`
      --covering <COVERINGS>
          Generate covering index variants by expanding column prefixes and direction combinations (use `|` to separate directions)
  -m, --schema <SCHEMA>
          Optional schema file describing column types
      --limit <LIMIT>
          Limit number of rows to scan (useful for prototyping)
      --delimiter <DELIMITER>
          CSV delimiter character (supports ',', 'tab', ';', '|')
      --input-encoding <INPUT_ENCODING>
          Character encoding of the input file (defaults to utf-8)
  -h, --help
          Print help
```

For advanced patterns (multi-variant specs, covering expansion, prefix/remainder sorting behavior, and performance guidance) see the extended guide: `docs/indexing-and-sorting.md`.

## process

```text
Transform a CSV file using sorting, filtering, projection, derivations, and schema-driven replacements

Usage: csv-managed.exe process [OPTIONS] --input <INPUT>

Options:
  -i, --input <INPUT>
          Input CSV file to process
  -o, --output <OUTPUT>
          Output CSV file (stdout if omitted)
  -m, --schema <SCHEMA>
          Schema file to drive typed operations and apply value replacements
  -x, --index <INDEX>
          Existing index file to speed up operations
      --index-variant <INDEX_VARIANT>
          Specific index variant name to use from the selected index file
      --sort <SORT>
          Sort directives of the form `column[:asc|desc]`
  -C, --columns <COLUMNS>
          Restrict output to this comma-separated list of columns
      --exclude-columns <EXCLUDE_COLUMNS>
          Exclude this comma-separated list of columns from output
      --derive <DERIVES>
          Additional derived columns using `name=expression`
      --filter <FILTERS>
          Row-level filters such as `amount>=100` or `status = shipped`
      --filter-expr <FILTER_EXPRS>
          Evalexpr-based filter expressions that must evaluate to truthy values
      --row-numbers
          Emit 1-based row numbers as the first column
      --limit <LIMIT>
          Limit number of rows emitted
      --delimiter <DELIMITER>
          CSV delimiter character for reading input
      --output-delimiter <OUTPUT_DELIMITER>
          Delimiter to use for output (defaults to input delimiter)
      --input-encoding <INPUT_ENCODING>
          Character encoding of the input file (defaults to utf-8)
      --output-encoding <OUTPUT_ENCODING>
          Character encoding for the output file/stdout (defaults to utf-8)
      --boolean-format <BOOLEAN_FORMAT>
          Normalize boolean columns in output [default: original] [possible values: original, true-false, one-zero]
      --apply-mappings
          Apply schema-defined datatype mappings before replacements (automatic when mappings exist)
      --skip-mappings
          Skip schema-defined datatype mappings even if they are present
      --preview
          Render results as a preview table on stdout (disables --output and defaults the row limit)
      --table
          Render output as an elastic table to stdout
  -h, --help
          Print help
```

Use `--apply-mappings` (enabled automatically when mappings exist) to run decimal rounding or truncation steps before values are written or validated.

## append

```text
Append multiple CSV files into a single output

Usage: csv-managed.exe append [OPTIONS] --input <INPUTS>

Options:
  -i, --input <INPUTS>
          One or more CSV files to append
  -o, --output <OUTPUT>
          Destination CSV file (stdout if omitted)
  -m, --schema <SCHEMA>
          Schema file to verify against
      --delimiter <DELIMITER>
          CSV delimiter character
      --input-encoding <INPUT_ENCODING>
          Character encoding for input files (defaults to utf-8)
      --output-encoding <OUTPUT_ENCODING>
          Character encoding for the output file/stdout (defaults to utf-8)
  -h, --help
          Print help
```

## stats

```text
Produce summary statistics for numeric columns or frequency counts via --frequency

Usage: csv-managed.exe stats [OPTIONS] --input <INPUT>

Options:
  -i, --input <INPUT>
          Input CSV file to profile
  -m, --schema <SCHEMA>
          Schema file to drive typed operations
  -C, --columns <COLUMNS>
          Columns to include (defaults to numeric columns)
      --filter <FILTERS>
          Row-level filters such as `amount>=100` or `status = shipped`
      --filter-expr <FILTER_EXPRS>
          Evalexpr-based filter expressions that must evaluate to truthy values
      --delimiter <DELIMITER>
          CSV delimiter character
      --input-encoding <INPUT_ENCODING>
          Character encoding for input file (defaults to utf-8)
      --limit <LIMIT>
          Maximum rows to scan (0 = all) [default: 0]
      --frequency
          Emit distinct value counts instead of summary statistics
      --top <TOP>
          Maximum distinct values to display per column when --frequency is used (0 = all) [default: 0]
  -h, --help
          Print help
```

## install

```text
Install the csv-managed binary via cargo install

Usage: csv-managed.exe install [OPTIONS]

Options:
      --version <VERSION>  Install a specific published version
      --force              Force reinstallation even if already installed
      --locked             Use --locked to honour Cargo.lock for dependencies
      --root <ROOT>        Install into an alternate root directory
  -h, --help               Print help
```
