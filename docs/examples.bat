setlocal

set "ROOT=%CD%"
set "BIN_RELEASE=%ROOT%\target\release\csv-managed.exe"
set "BIN_DEBUG=%ROOT%\target\debug\csv-managed.exe"
set "CSV_BIG5=%ROOT%\tests\data\big_5_players_stats_2023_2024.csv"
set "SCHEMA_BIG5=%ROOT%\tests\data\big_5_players_stats-schema.yml"
set "STATS_CSV=%ROOT%\tests\data\stats_schema.csv"
set "STATS_SCHEMA=%ROOT%\tests\data\stats_schema-schema.yml"

if not exist .\tmp mkdir .\tmp

rem Schema command examples
REM Wrap replacement specifications in quotes so the shell does not treat '>' as redirection
.\target\release\csv-managed.exe schema -o .\tmp\schema_basic-schema.yml -c "id:integer" -c "name:string" -c "amount:float"
::type .\tmp\schema_basic-schema.yml
.\target\release\csv-managed.exe schema -o .\tmp\schema_alias-schema.yml -c "status:string->order_status" -c "created_at:datetime" --replace "status=pending->ready" --replace "status=unknown->ready"
::type .\tmp\schema_alias-schema.yml
.\target\release\csv-managed.exe schema -o .\tmp\schema_list-schema.yml -c "id:integer,status:string,created_at:datetime"
::type .\tmp\schema_list-schema.yml

.\target\release\csv-managed.exe index -i .\tests\data\orders_temporal.csv -o .\tmp\orders_temporal_covering.idx -m .\tests\data\orders_temporal-schema.yml ^
  --covering "orders=ordered_at:asc|desc,status:asc"

rem Demonstrate majority-based inference recovering non-string types from noisy data
.\target\release\csv-managed.exe schema probe -i .\tests\data\majority_datatypes.csv --sample-rows 0

rem Probe a headerless CSV; inferred columns use field_# names
.\target\release\csv-managed.exe schema probe -i .\tests\data\sensor_readings_no_header.csv --mapping --sample-rows 0

rem Index command examples
rem Build a simple ascending index over the order timestamp column
.\target\release\csv-managed.exe index -i .\tests\data\orders_temporal.csv -o .\tmp\orders_temporal_ordered_at.idx -m .\tests\data\orders_temporal-schema.yml -C ordered_at

rem Create mixed-direction index variants with explicit specifications
.\target\release\csv-managed.exe index -i .\tests\data\orders_temporal.csv -o .\tmp\orders_temporal_variants.idx -m .\tests\data\orders_temporal-schema.yml ^
  --spec "recent=ordered_at:desc" --spec "ordered_at:asc,ship_time:desc"

rem Use a named index variant to accelerate a descending ordered_at sort
.\target\release\csv-managed.exe process -i .\tests\data\orders_temporal.csv -m .\tests\data\orders_temporal-schema.yml ^
  -x .\tmp\orders_temporal_variants.idx --index-variant recent --sort ordered_at:desc --columns ordered_at --columns status --limit 10 --preview

.\target\release\csv-managed.exe process -i .\tests\data\orders_temporal.csv -m .\tests\data\orders_temporal-schema.yml ^
  -x .\tmp\orders_temporal_variants.idx --index-variant recent --sort ordered_at:desc --limit 10 --preview

rem Expand a covering spec to generate prefix indexes for multiple direction combinations
.\target\release\csv-managed.exe index -i .\tests\data\orders_temporal.csv -o .\tmp\orders_temporal_covering.idx -m .\tests\data\orders_temporal-schema.yml ^
  --covering "orders=ordered_at:asc|desc,status:asc"

rem Prototype an index from a subset of rows to validate column choices quickly
.\target\release\csv-managed.exe index -i .\tests\data\big_5_players_stats_2023_2024.csv -o .\tmp\big5_players_perf.idx -m .\tests\data\big_5_players_stats-schema.yml ^
  --spec "Performance_Gls:desc,Performance_Ast:desc" --limit 1000

rem List columns from a saved schema file
.\target\release\csv-managed.exe schema columns --schema .\tests\data\orders-schema.yml
.\target\release\csv-managed.exe schema columns --schema .\tests\data\big_5_players_stats-schema.yml

rem Probe full file (all rows) to capture mixed column as string
.\target\release\csv-managed.exe schema infer -i .\tests\data\probe_sample_variation.csv -o .\tmp\probe_full-schema.yml --sample-rows 0

rem Inspect a handful of rows and review inferred samples, format hints, and override status directly in the console:
.\target\release\csv-managed.exe schema probe -i tests/data/big_5_players_stats_2023_2024.csv --sample-rows 0
.\target\release\csv-managed.exe schema probe -i tests/data/big_5_players_stats_2023_2024.csv --mapping --sample-rows 250

rem Generate a schema file populated with snake_case renames and empty replacement arrays so you can fill in value substitutions later:
.\target\release\csv-managed.exe schema infer --mapping --replace-template -i tests/data/big_5_players_stats_2023_2024.csv -o tmp/big5_inferred-schema.yml --sample-rows 0

rem Preview inferred schema YAML and probe report without writing the output file (dry run)
.\target\release\csv-managed.exe schema infer --mapping --replace-template ^
  -i tests/data/big_5_players_stats_2023_2024.csv -o tmp/big5_preview-schema.yml --sample-rows 0 --preview

rem Show differences between the current inference and a saved schema file without overwriting it
.\target\release\csv-managed.exe schema infer -i tests/data/big_5_players_stats_2023_2024.csv --sample-rows 0 --diff .\tmp\big5_inferred-schema.yml

rem Probe with limited sampling to infer integer type from first row only
.\target\release\csv-managed.exe schema infer -i .\tests\data\probe_sample_variation.csv -o .\tmp\probe_sampled-schema.yml --sample-rows 1

rem This command performs a full scan (`--sample-rows 0`) before writing `tmp/big5_inferred-schema.yml`.
.\target\release\csv-managed.exe schema infer --mapping --override Performance_Gls:integer --override "Per 90 Minutes_Gls:string" ^
  -i tests/data/big_5_players_stats_2023_2024.csv -o tmp/big5_overrides-schema.yml --sample-rows 10

rem Capture a schema probe snapshot with header/type hash and sampled value summaries for regression review
.\target\release\csv-managed.exe schema probe -i .\tests\data\big_5_players_stats_2023_2024.csv --sample-rows 10 --snapshot .\tmp\big5_probe.snap

rem Validate the snapshot by rerunning infer; this fails on header/type drift
.\target\release\csv-managed.exe schema infer -i .\tests\data\big_5_players_stats_2023_2024.csv --sample-rows 0 --snapshot .\tmp\big5_probe.snap

rem Create a Windows-1252 encoded CSV derived from the Big 5 stats dataset
.	arget\release\csv-managed.exe process -i .\tests\data\big_5_players_stats_2023_2024.csv -m .\tests\data\big_5_players_stats-schema.yml --limit 25 --output .\tmp\big_5_windows1252.csv --output-encoding windows-1252

rem (fixed path reference for above command; retained original line for historical context)
.\target\release\csv-managed.exe process -i .\tests\data\big_5_players_stats_2023_2024.csv -m .\tests\data\big_5_players_stats-schema.yml --limit 25 --output .\tmp\big_5_windows1252.csv --output-encoding windows-1252

rem Probe using explicit input encoding support
.\target\release\csv-managed.exe schema infer -i .\tmp\big_5_windows1252.csv -o .\tmp\probe_windows-schema.yml --input-encoding windows-1252

.\target\release\csv-managed.exe schema infer -i .\tests\data\datatype_mapping.csv -o .\tmp\datatype_mapping-schema.yml --mapping --replace-template
.\target\release\csv-managed.exe schema infer -i .\tests\data\orders_invalid.csv -o .\tmp\orders-schema.yml --mapping --replace-template
.\target\release\csv-managed.exe schema infer -i .\tests\data\orders_temporal.csv -o .\tmp\orders_temporal-schema.yml --mapping --replace-template
.\target\release\csv-managed.exe schema infer -i .\tests\data\stats_schema.csv -o .\tmp\stats_schema-schema.yml --mapping --replace-template
.\target\release\csv-managed.exe schema infer -i .\tests\data\stats_temporal.csv -o .\tmp\stats_temporal-schema.yml --mapping --replace-template

.\target\release\csv-managed.exe schema infer -i .\tests\data\big_5_players_stats_2023_2024.csv -o .\tests\data\big_5_players_stats-schema.yml --mapping --replace-template
.\target\release\csv-managed.exe schema verify -m .\tests\data\big_5_players_stats-schema.yml -i .\tests\data\big_5_players_stats_2023_2024.csv
.\target\release\csv-managed.exe schema verify -m .\tests\data\orders-schema.yml -i .\tests\data\orders_invalid.csv --report-invalid


rem Datatype mapping feature: reuse sample data and schema from tests/data
.\target\release\csv-managed.exe schema columns --schema .\tests\data\datatype_mapping-schema.yml
.\target\release\csv-managed.exe schema verify -m .\tests\data\datatype_mapping-schema.yml -i .\tests\data\datatype_mapping.csv --report-invalid

rem Apply mappings and view normalized values (date truncation, float rounding, lowercase + replacement)
.\target\release\csv-managed.exe process -i .\tests\data\datatype_mapping.csv -m .\tests\data\datatype_mapping-schema.yml --apply-mappings --preview
.\target\release\csv-managed.exe process -i .\tests\data\datatype_mapping.csv -m .\tests\data\datatype_mapping-schema.yml --apply-mappings -o .\tmp\datatype_mapping_clean.csv
type .\tmp\datatype_mapping_clean.csv

rem Currency datatype coverage: enforce rounding, truncation, and scale validation
.\target\release\csv-managed.exe schema columns --schema .\tests\data\currency_transactions-schema.yml
.\target\release\csv-managed.exe schema verify -m .\tests\data\currency_transactions-schema.yml -i .\tests\data\currency_transactions.csv
.\target\release\csv-managed.exe schema verify -m .\tests\data\currency_transactions-schema.yml -i .\tests\data\currency_transactions_invalid.csv --report-invalid
.\target\release\csv-managed.exe process -i .\tests\data\currency_transactions.csv -m .\tests\data\currency_transactions-schema.yml --apply-mappings -o .\tmp\currency_transactions_clean.csv
type .\tmp\currency_transactions_clean.csv

rem Fixed decimal datatype coverage: configure precision/scale and rounding strategies
.\target\release\csv-managed.exe schema columns --schema .\tests\data\decimal_measurements-schema.yml
.\target\release\csv-managed.exe schema verify -m .\tests\data\decimal_measurements-schema.yml -i .\tests\data\decimal_measurements.csv --report-invalid
.\target\release\csv-managed.exe process -i .\tests\data\decimal_measurements.csv -m .\tests\data\decimal_measurements-schema.yml --apply-mappings -o .\tmp\decimal_measurements_clean.csv
type .\tmp\decimal_measurements_clean.csv

rem Compute statistics and frequency counts on transformed data (mappings run automatically)
.\target\release\csv-managed.exe stats -i .\tests\data\datatype_mapping.csv -m .\tests\data\datatype_mapping-schema.yml --columns amount
.\target\release\csv-managed.exe stats -i .\tests\data\datatype_mapping.csv -m .\tests\data\datatype_mapping-schema.yml --frequency -C status

:: To observe the failure mode without datatype mappings, uncomment the command below (order_ts will not parse as a date)
:: .\target\release\csv-managed.exe process -i .\tmp\datatype_mapping.csv -m .\tmp\datatype_mapping-schema.yml --skip-mappings --preview

.\target\debug\csv-managed.exe schema verify -m .\tests\data\orders-schema.yml -i .\tests\data\orders_invalid.csv --report-invalid
.\target\debug\csv-managed.exe schema verify -m .\tests\data\orders-schema.yml -i .\tests\data\orders_invalid.csv --report-invalid:detail 5
.\target\debug\csv-managed.exe schema verify -m .\tests\data\orders-schema.yml -i .\tests\data\orders_invalid.csv --report-invalid:detail:summary

rem Prepare preview subsets derived from the Big 5 stats dataset
.\target\release\csv-managed.exe process -i .\tests\data\big_5_players_stats_2023_2024.csv --limit 15 --columns Rank --columns Player --columns Squad -o .\tmp\big_5_preview.csv
.\target\release\csv-managed.exe process -i .\tests\data\big_5_players_stats_2023_2024.csv --limit 5 --columns Rank --columns Player --columns Squad --output-delimiter tab -o .\tmp\big_5_preview.tsv
.\target\release\csv-managed.exe process -i .\tests\data\big_5_players_stats_2023_2024.csv --limit 5 --columns Rank --columns Player --columns Squad --output-delimiter "|" -o .\tmp\big_5_preview_pipe.csv

rem Preview default row count (10 rows) from the primary dataset
.\target\release\csv-managed.exe process -i .\tests\data\big_5_players_stats_2023_2024.csv --preview

rem Preview with an explicit row limit
.\target\release\csv-managed.exe process -i .\tests\data\big_5_players_stats_2023_2024.csv --preview --limit 5

rem Preview auto-detects tab-delimited files via extension
.\target\release\csv-managed.exe process -i .\tmp\big_5_preview.tsv --preview

rem Preview with a custom delimiter override (pipe-separated values)
.\target\release\csv-managed.exe process -i .\tmp\big_5_preview_pipe.csv --preview --delimiter "|"

rem Preview using explicit input encoding for Windows-1252 data
.\target\release\csv-managed.exe process -i .\tmp\big_5_windows1252.csv --preview --input-encoding windows-1252

rem Process with derived columns, filters, and table output
.\target\release\csv-managed.exe process -i .\tests\data\sort_types.csv -m .\tests\data\sort_types-schema.yml ^
  --filter "bool_col=true" --filter-expr "float_col>=0.0 and int_col>=0" --derive "double_int=int_col*2" --exclude-columns guid_col ^
  --row-numbers --boolean-format true-false --table --limit 5

rem Export filtered results with custom encoding and boolean normalization
.\target\release\csv-managed.exe process -i .\tests\data\sort_types.csv -m .\tests\data\sort_types-schema.yml ^
  --filter "bool_col=false" --columns id --columns bool_col --columns currency_col --columns decimal_col ^
  --boolean-format one-zero --output .\tmp\sort_types_filtered.csv --output-encoding windows-1252
type .\tmp\sort_types_filtered.csv

rem Prepare subsets for append command demonstrations
.\target\release\csv-managed.exe process -i .\tests\data\orders_temporal.csv -m .\tests\data\orders_temporal-schema.yml ^
  --columns id --columns ordered_at --columns status --filter "status=shipped" -o .\tmp\orders_shipped.csv
.\target\release\csv-managed.exe process -i .\tests\data\orders_temporal.csv -m .\tests\data\orders_temporal-schema.yml ^
  --columns id --columns ordered_at --columns status --filter "status=cancelled" -o .\tmp\orders_cancelled.csv

rem Append command examples
.\target\release\csv-managed.exe append -i .\tmp\orders_shipped.csv -i .\tmp\orders_cancelled.csv ^
  --schema .\tests\data\orders_temporal-schema.yml --output .\tmp\orders_combined.csv
type .\tmp\orders_combined.csv

rem Verify multiple files against the same schema with summary reporting
.\target\release\csv-managed.exe schema verify -m .\tests\data\orders_temporal-schema.yml ^
  -i .\tmp\orders_shipped.csv -i .\tmp\orders_cancelled.csv -i .\tmp\orders_combined.csv --report-invalid:summary

rem Stats command examples
.\target\release\csv-managed.exe stats -i .\tests\data\stats_infer.csv
.\target\release\csv-managed.exe stats -i .\tests\data\stats_schema.csv -m .\tests\data\stats_schema-schema.yml --columns price
.\target\release\csv-managed.exe stats -i .\tests\data\stats_schema.csv -m .\tests\data\stats_schema-schema.yml --columns quantity --limit 2
.\target\release\csv-managed.exe stats -i .\tests\data\stats_temporal.csv -m .\tests\data\stats_temporal-schema.yml --columns ordered_at --columns ordered_at_ts --columns ship_time
.\target\release\csv-managed.exe stats -i .\tests\data\stats_schema.csv -m .\tests\data\stats_schema-schema.yml --frequency --top 5
.\target\release\csv-managed.exe stats -i .\tests\data\big_5_players_stats_2023_2024.csv --frequency -C Squad --filter "Player=Max Aarons"
.\target\release\csv-managed.exe stats -i .\tests\data\sort_types.csv -m .\tests\data\sort_types-schema.yml --filter "bool_col=true" --filter-expr "float_col>=0.0" --limit 10

rem String transform derive example
.\target\release\csv-managed.exe process -i .\tests\data\big_5_players_stats_2023_2024.csv -m .\tests\data\big_5_players_stats-schema.yml ^
  --derive "slug=snake_case(Player)" --derive "camel_name=camel_case(Player)" ^
  --columns Player --limit 5 --table

rem Streaming derive + schema evolution emission example (process -> stats)
set "DERIVED_SCHEMA=%ROOT%\tmp\stats_with_extra-schema.yml"
set "DERIVED_EVOLUTION=%ROOT%\tmp\stats_with_extra-schema.evo.yml"
type "%STATS_CSV%" | "%BIN_RELEASE%" process -i - --schema "%STATS_SCHEMA%" ^
  --derive "double_price:Float=price*2" --emit-schema "%DERIVED_SCHEMA%" --emit-evolution-base "%STATS_SCHEMA%" | ^
"%BIN_RELEASE%" stats -i - --schema "%DERIVED_SCHEMA%" -C double_price
type "%DERIVED_EVOLUTION%"

rem -------------------------------------------------------------
rem Streaming & Pipelines (stdin '-') Examples
rem -------------------------------------------------------------

rem Process via stdin using type for batch
type "%CSV_BIG5%" | "%BIN_RELEASE%" process -i - --schema "%SCHEMA_BIG5%" --columns Player --columns Performance_Gls --limit 5 --table

rem Chain process -> stats (filter rows then compute stats) via stdin
rem NOTE: Place the pipe (|) at the end of the line to avoid issues with line continuation
rem Pipe the first output to csv-managed.exe on the first line as in the example below to avoid line continuation issues.
type "%CSV_BIG5%" | "%BIN_RELEASE%" process -i - --schema "%SCHEMA_BIG5%" --filter "Performance_Gls>=10" --limit 40 | ^
"%BIN_RELEASE%" stats -i - --schema "%SCHEMA_BIG5%" -C Performance_Gls

rem Mixed streaming + file input (append)
type "%CSV_BIG5%" | "%BIN_RELEASE%" append -i - -i .\tmp\big_5_preview.csv --schema "%SCHEMA_BIG5%" -o .\tmp\players_union.csv

rem Filter upstream then stats downstream
type "%CSV_BIG5%" | "%BIN_RELEASE%" process -i - --schema "%SCHEMA_BIG5%" --filter "Performance_Gls>=5" | ^
"%BIN_RELEASE%" stats -i - --schema "%SCHEMA_BIG5%" -C Performance_Gls

rem Minimal (string-only) pipeline without schema (not recommended for typed ops)
type "%CSV_BIG5%" | "%BIN_RELEASE%" process -i - --columns Player --limit 3 --table

rem End Streaming & Pipelines examples
rem Encoding normalization + stats pipeline (Windows-1252 -> UTF-8)
rem NOTE: Include columns required for downstream stats; header structure must remain consistent with schema.
type .\tmp\big_5_windows1252.csv | ^
.\target\release\csv-managed.exe process -i - --input-encoding windows-1252 --schema .\tests\data\big_5_players_stats-schema.yml ^
  --columns Player --columns Squad --columns Performance_Gls --limit 40 | ^
.\target\release\csv-managed.exe stats -i - --schema .\tests\data\big_5_players_stats-schema.yml -C Performance_Gls
rem Header mapping note: downstream typed stages accept either original or mapped column names when only name_mapping (rename) is applied without structural changes.

endlocal
