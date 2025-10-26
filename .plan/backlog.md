# TO-DO

## Version 1.0.0

- [x] Provide a B-Tree indexing command that writes `.idx` files for selected columns.
- [x] Allow subsequent commands to select a prebuilt index file (and variant) during processing.
- [x] Support sorting CSV output by specified columns, including columns present in an index.
- [x] Support ascending and descending sort directions across multiple columns.
- [x] Allow explicit column name and datatype declarations to be stored in `-schema.yml` files.
- [x] Probe CSV files to infer column datatypes and persist them to a schema file.
- [x] Emit sorted results to new files via the `-o/--output` argument.
- [x] Add row numbers to output rows when requested.
- [x] Select a subset of columns (or exclude specific columns) from either the original or transformed data.
- [x] Derive or add new columns via expression evaluation.
- [x] Filter rows using column-aware comparisons and expression-based predicates.
- [x] Normalize boolean values to standard `true/false` or `1/0` forms.
- [x] Append data from multiple CSV files into a single output while enforcing schema consistency.
- [x] Verify the integrity of one or more CSV files against a schema definition.
- [x] Verify multiple CSV files against a single schema, including during append operations.
- [x] Stream processing to minimize memory usage and maximize throughput.
- [x] Accept stdin/stdout for chaining with other command-line utilities.
- [x] Project new columns (including boolean flags) from expressions applied to existing columns.
- [x] Join two CSV files with inner, left, right, and full outer joins.
- [x] Render filtered results as elastic tables without writing an intermediate file.
- [x] Preview a limited number of rows from the start of a CSV file.
- [x] Produce summary statistics (count, mean, median, min, max, standard deviation) for numeric columns.
- [x] Produce frequency counts for categorical columns.
- [x] Support excluding columns from the output file.
- [x] Install the published binary via `cargo install` for easy CLI access.
- [x] Index a file on multiple column combinations, including mixed sort directions, and store multiple variants.
- [x] List schema columns and datatypes in a human-readable console table.
- [x] Persist schema rename mappings so outputs can remap column names consistently.
- [x] Output a schema definition in a human-readable format on the console.
- [x] Change date, time, or datetime formats to custom string outputs.
- [x] Replace column values according to schema-defined mappings before processing.
- [x] Highlight schema violations with row-level samples and column summaries in verification reports.
- [x] Review current index storage format and limitations for multi-column/direction support.
- [x] Design structure for storing multiple index variants keyed by column names and sort directions.
- [x] Update index build logic to optionally create several variants in one file or multiple files.
- [x] Extend index serialization format to capture direction metadata per column.
- [x] Update CLI to allow naming/selecting specific index variants with direction info.
- [x] Teach process command to pick best-matching index variant based on requested sorts, including mixed directions.
- [x] Implement install command that shells out to `cargo install csv-managed` with options for version/force/local path.
- [x] Document install command usage and multi-index support in README.
- [x] Add tests covering install command argument plumbing and multi-index selection logic.
- [x] Add GUID data type support.
- [x] Refactor the solution to use -schema.yml instead of .meta and rename metadata.rs to schema.rs.
- [x] Add the ability to specify the column names along with the data types of a CSV file in the schema file.
- [x] Add the ability to specify in the -schema.yml file a mapping of existing column names to new column names to be used in all outputs from the file.
- [x] Add GitHub pipeline build and deployment capability using build and deployment definitions or actions.
- [x] Add deployment of the executable as a binary for easy access from the command line from the cargo package store.
- [x] Add the ability to index a file on multiple combinations of columns and store multiple indexes for the same file and mixed sort directions (ascending/descending) per column.
- [x] Add the ability to list column names and data types as a list to the console output.
- [x] Add the ability to emit column mapping templates for each column in a file to the probe command.
- [x] Refactor the probe and schema commands to use "datatype" instead of "data_type" in the schema file.
- [x] Add to the probe command the functionality to inject a "replace" node that holds an empty array as a template for future replace functionality.
- [x] Add the ability to replace values by column in the original input file; the schema file should allow you to define multiple value/replace pairs per column in a node of the column named "replace", which holds an array of value and replacement value pairs; this feature runs through the `process` command.
- [x] Add timestamps to the output of all operations such that the output after the completion of an operation should include the start date/time, end date/time, and duration in seconds.
- [x] Add to the verify command a flag to print out all rows or a specified limit of rows that do not fit the schema, highlighting in red the values that do not fit the schema definition for the column, indicating the row number and column.  These should be printed out to the console window in an elastic tab formatted table.  At the end of the console printout, another table should be printed of the columns with errors and their schema defined data types.

## Version 1.1.0: The Stats, Schema, Datatype, and Transformation Edition (Core Operations)

- [x] Add the ability in filtering & projection and column derivation to perform date, time, and datetime logic with Evalexpr-powered expressions.
- [x] Add the ability to output the schema definition for a CSV file in a human-readable list format to the console output.
- [x] Add the ability to perform stats over date, time, and datetime values.
- [x] Refactor the frequency command features & capabilities to be incorporated into the stats command and subsequently remove the frequency command to simplify the command-line interface; implement as stats --frequency.
- [x] Add the ability to apply a filter on the stats commmand to filter the rows on which stats are being calculated, including for use with the --frequency flag.
- [x] Add a --filter feature to the stats command to filter the rows on which stats are being calculated; filter should operate as a forward-only, in-place read of the data and should not require temporary files.
- [x] Refactor the probe command into the schema command to simplify the command-line interface: probe and inference should become new subcommands of the schema command that allow it to probe and display candidate schema definitions and to infer data types into a schema file; inference should allow the user to override an inferred type with extra arguments.
- [x] Refactor the verify command into a subcommand of the schema command to simplify the command-line interface; implement as schema verify.
- [x] Refactor the columns command into a subcommand of the schema command to simplify the command-line interface; implement as schema columns.
- [x] Refactor the preview command into the process command as a flag to simplify the command-line interface; implement as process --preview.  When --preview is added to the process command, no file output should be allowed as the intended purpose is to preview the processing output only to the console window.
- [x] Remove access to the join command; do not remove core code, just the CLI interface to that code that allows that feature to be used.  This feature will be further developed in a later release cycle.
- [x] Expand the schema command:
  - [x] Extend schema infer --snapshot to emit the header-order hash and datatype map.
  - [x] Add sampled-value summaries (per-column histograms or min/max).
  - [x] Append a “Snapshot Internals” subsection to the README.
  - [x] Add a cross-link from the schema command section near the --snapshot flag to the new comparison section.
- [x] Plan out a datatype_mapping feature to the schema file and the ability to transform one data type to another where possible.  Implement the plan.
- [x] Refactor the solution to implement schema files in YAML format rather than JSON.  Create the plan first and share with me for review.  Once approved, implement the plan along with any adjustments requested from me.
- [x] Add the ability to transform fields between datatypes (e.g., string → date/datetime) in a controlled manner; schema file should support datatype definitions for original/source and target datatypes.
- [x] Add the ability to perform multi-step transformations on a field and to define those multi-step transformations in the schema file.
- [x] Add a currency datatype with enforced precision and standardized formatting/transforms that restricts decimal precision to 2 or 4 digits to the right of the decimal point, probing for valid currency formats and ensuring correct parsing and validation.  Also the ability to transform a longer decimal or float value to a currency format for data standardization.
- [x] Support fixed scale and precision decimal datatypes with configurable scale and precision.  Update documentation to describe this capability. Include command-line examples in examples.bat that demonstrate this feature.
- [x] Support sorting by every listed datatype, including high-precision decimal values.
- [x] Update the code to use the most recent versions of the package dependencies listed in the Cargo.toml file; refactorings will be required since there are breaking changes in some of the new versions.  First develop a plan and confirm that it will not break the application.  Validate if that code updates can support the latest version of Rust.
- [x] Enhance the schema probe and infer commands to detect and recommend specific non-string datatypes based on the majority of values in the set for declaration in a schema file.
- [ ] Add a feature in probing and inference processing data to detect and treat variations of NA or N/A or #N/A or #NA as null or empty; provide options for treating as empty or to fill with value like "null" or "NULL."  Probe command should suggest replacements in schema file for these scenarios, especially where the detected datatype is not String.

## Version 1.1.5: Upgrade serde_yaml dependency (spike)

- [ ] Spike a migration to one of the alternatives for serde_yaml: serde_yaml_ng, serde_yaml_ok, serde_yml.

## Version 1.2.0: The Index & Benchmark Edition

- [ ] Spike a migration to one of the alternatives for serde_yaml: serde_yaml_ng, serde_yaml_ok, serde_yml.
- [ ] Define primary keys (single or composite) that uniquely identify rows.
- [ ] Add fast hash signatures per row for indexes defined on primary keys.
- [ ] Index all files in a directory and subdirectories that share a schema definition.
- [ ] Add the ability to index all of the files in a directory and subdirectories matching a single schema file.
- [ ] Add the ability to define a primary key (single column or composite) that uniquely identifies each row in a CSV file and expose it through CLI workflows.
- [ ] Add the ability to add a fast hash signature for each row in primary-key-backed indexes.
- [ ] Add the ability to probe a file for candidate primary/composite keys and report them to the console.

## Version 1.3.0: Bulk Operations Edition

- [ ] Add the ability to consume a batch processing definition file in which all possible command-line arguments can be defined; file should be in YAML format.
- [ ] Consume a YAML batch definition describing command-line arguments for automated runs.
- [ ] Add the ability process delimited files with no header; needs to understand column position logic including in schema files; should be able to map column positions to virtual column names defined in the schema file but not in the delimited file; should also be able to process delimited files with no header to files with headers defined in schema file's virtual column names.
- [ ] Add the ability to point the app at all files of the same file extension in a directory and verify each file against a -schema.yml file schema definition including data type verification.
- [ ] Union and sort all files in a directory, splitting output by row count or file size.
- [ ] Perform a union across multiple files with deduplication.
- [ ] Probe files to suggest candidate primary (or composite) keys.
- [ ] Verify all files in a directory (by extension) against a shared schema in one operation.
- [ ] Enhance verification capabilities for cloud-scale data validation scenarios.
- [ ] Add the ability to union all of the files in a directory in a sorted order and split into multiple files based on either row count per file or file size.
- [ ] Add the ability to perform a union of multiple files that is able to deduplicate rows across multiple files and output to a single file.
- [ ] Add the ability to perform the stats command over a series of files all conforming to the same schema definition, most likely in a directory or across a set of subdirectories.
- [ ] Need to expand multiple commands to process file data across multiple files and subdirectories where all files conform to a single schema file.

## Version 1.4.0: The Excel & JSON File Edition

- [ ] Add the ability to process Excel data, streaming rows from selected worksheet(), feeding them through existing schema/replacement/projection machinery; implements data normalization of Excel formatted data.

## Version 1.5.0: The Parquet File Edition

- [ ] Add the ability to read Parquet files.
- [ ] Create plan for implementing efficient Parquet file indexing and data access.  Version 1.5 needs a full product feature plan and strategy.

## Version 1.6.0: The JOIN Edition

## Version 1.7.0: The Performance Edition

## Backlog

- [ ] Enhance data verification and reporting capabilities to support cloud-hosted validation scenarios (multi-tenant, large scale, granular reporting).
- [ ] Develop example GitHub Copilot prompts demonstrating how to direct an AI agent to plan out and generate a set of command-line actions to achieve a range of data wrangling outcomes.  Add prompts as a new set of documentation.
- [ ] Consider creating a new version that focuses on locale specific input/output.
