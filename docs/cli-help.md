# CLI Help (Regenerated 2025-10-21)

Captured with `cargo run -- <subcommand> --help` on Windows PowerShell.

## Global Help

```text
Manage CSV files efficiently

Usage: csv-managed.exe <COMMAND>

Commands:
  probe    Probe a CSV file and infer column data types into a .schema file
  schema   Create a .schema file from explicit column definitions
  index    Create a B-Tree index (.idx) for one or more columns
  process  Transform a CSV file using sorting, filtering, projection, derivations, and schema-driven replacements
  append   Append multiple CSV files into a single output
  verify   Verify one or more CSV files against a schema definition
  preview  Preview the first few rows of a CSV file in a formatted table
  stats    Produce summary statistics for numeric columns or frequency counts via --frequency
  join     Join two CSV files on common columns
  install  Install the csv-managed binary via cargo install
  columns  List column names and data types from a schema file
  help     Print this message or the help of the given subcommand(s)

Options:
  -h, --help     Print help
  -V, --version  Print version
```

## probe

```text
Probe a CSV file and infer column data types into a .schema file

Usage: csv-managed.exe probe [OPTIONS] --input <INPUT> --schema <SCHEMA>

Options:
  -i, --input <INPUT>
          Input CSV file to inspect
  -m, --schema <SCHEMA>
          Destination .schema file path
      --sample-rows <SAMPLE_ROWS>
          Number of rows to sample when inferring types (0 means full scan) [default: 2000]
      --delimiter <DELIMITER>
          CSV delimiter character (supports ',', 'tab', ';', '|')
      --input-encoding <INPUT_ENCODING>
          Character encoding of the input file (defaults to utf-8)
      --mapping
          Emit column mapping templates to stdout after probing
      --replace
          Inject empty replace arrays into the generated schema as a template
  -h, --help
          Print help
```

## schema

```text
Create a .schema file from explicit column definitions

Usage: csv-managed.exe schema [OPTIONS] --output <OUTPUT> --column <COLUMNS>

Options:
  -o, --output <OUTPUT>         Destination .schema file path
  -c, --column <COLUMNS>        Column definitions using `name:type` syntax (comma-separated or repeatable)
      --replace <REPLACEMENTS>  Value replacement directives using `column=value->replacement`
  -h, --help                    Print help
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
      --combo <COMBOS>
          Generate index variants by expanding column prefixes and direction combinations (use `|` to separate directions)
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
      --table
          Render output as an elastic table to stdout
  -h, --help
          Print help
```

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

## verify

```text
Verify one or more CSV files against a schema definition

Usage: csv-managed.exe verify [OPTIONS] --schema <SCHEMA> --input <INPUTS>

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

## preview

```text
Preview the first few rows of a CSV file in a formatted table

Usage: csv-managed.exe preview [OPTIONS] --input <INPUT>

Options:
  -i, --input <INPUT>                    Input CSV file to preview
      --rows <ROWS>                      Number of rows to display [default: 10]
      --delimiter <DELIMITER>            CSV delimiter character
      --input-encoding <INPUT_ENCODING>  Character encoding for input file (defaults to utf-8)
  -h, --help                             Print help
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

## join

```text
Join two CSV files on common columns

Usage: csv-managed.exe join [OPTIONS] --left <LEFT> --right <RIGHT> --left-key <LEFT_KEY> --right-key <RIGHT_KEY>

Options:
      --left <LEFT>
          Left CSV input
      --right <RIGHT>
          Right CSV input
  -o, --output <OUTPUT>
          Output CSV file (stdout if omitted)
      --left-key <LEFT_KEY>
          Comma-separated key columns from the left file
      --right-key <RIGHT_KEY>
          Comma-separated key columns from the right file
      --type <KIND>
          Join type (inner, left, right, full) [default: inner] [possible values: inner, left, right, full]
      --left-schema <LEFT_SCHEMA>
          Schema for the left file
      --right-schema <RIGHT_SCHEMA>
          Schema for the right file
      --delimiter <DELIMITER>
          CSV delimiter character for inputs
      --left-encoding <LEFT_ENCODING>
          Character encoding for the left input file (defaults to utf-8)
      --right-encoding <RIGHT_ENCODING>
          Character encoding for the right input file (defaults to utf-8)
      --output-encoding <OUTPUT_ENCODING>
          Character encoding for the output file/stdout (defaults to utf-8)
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

## columns

```text
List column names and data types from a schema file

Usage: csv-managed.exe columns --schema <SCHEMA>

Options:
  -m, --schema <SCHEMA>  Schema file describing the columns to list
  -h, --help             Print help
```
