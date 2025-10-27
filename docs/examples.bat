if not exist .\tmp mkdir .\tmp

rem Schema command examples
REM Wrap replacement specifications in quotes so PowerShell/CMD do not treat '>' as redirection
.\target\release\csv-managed.exe schema -o .\tmp\schema_basic-schema.yml -c "id:integer" -c "name:string" -c "amount:float"
::type .\tmp\schema_basic-schema.yml
.\target\release\csv-managed.exe schema -o .\tmp\schema_alias-schema.yml -c "status:string->order_status" -c "created_at:datetime" --replace "status=pending->ready" --replace "status=unknown->ready"
::type .\tmp\schema_alias-schema.yml
.\target\release\csv-managed.exe schema -o .\tmp\schema_list-schema.yml -c "id:integer,status:string,created_at:datetime"
::type .\tmp\schema_list-schema.yml
.\target\release\csv-managed.exe schema -o .\tmp\schema_basic-schema.yml -c "id:integer" -c "name:string" -c "amount:float"
::type .\tmp\schema_basic-schema.yml
.\target\release\csv-managed.exe schema -o .\tmp\schema_alias-schema.yml -c "status:string->order_status" -c "created_at:datetime" --replace "status=pending->ready" --replace "status=unknown->ready"
::type .\tmp\schema_alias-schema.yml
.\target\release\csv-managed.exe schema -o .\tmp\schema_list-schema.yml -c "id:integer,status:string,created_at:datetime"
::type .\tmp\schema_list-schema.yml

rem Demonstrate majority-based inference recovering non-string types from noisy data
.\target\release\csv-managed.exe schema probe -i .\tests\data\majority_datatypes.csv --sample-rows 0

rem Index command examples
rem Build a simple ascending index over the order timestamp column
.\target\release\csv-managed.exe index -i .\tests\data\orders_temporal.csv -o .\tmp\orders_temporal_ordered_at.idx -m .\tests\data\orders_temporal-schema.yml -C ordered_at

rem Create mixed-direction index variants with explicit specifications
.\target\release\csv-managed.exe index -i .\tests\data\orders_temporal.csv -o .\tmp\orders_temporal_variants.idx -m .\tests\data\orders_temporal-schema.yml ^
  --spec "recent=ordered_at:desc" --spec "ordered_at:asc,ship_time:desc"

rem Use a named index variant to accelerate a descending ordered_at sort
.\target\release\csv-managed.exe process -i .\tests\data\orders_temporal.csv -m .\tests\data\orders_temporal-schema.yml ^
  -x .\tmp\orders_temporal_variants.idx --index-variant recent --sort ordered_at:desc --columns ordered_at --columns status --limit 10 --preview

rem Expand a combo spec to generate prefix indexes for multiple direction combinations
.\target\release\csv-managed.exe index -i .\tests\data\orders_temporal.csv -o .\tmp\orders_temporal_combo.idx -m .\tests\data\orders_temporal-schema.yml ^
  --combo "orders=ordered_at:asc|desc,status:asc"

rem Prototype an index from a subset of rows to validate column choices quickly
.\target\release\csv-managed.exe index -i .\tests\data\big_5_players_stats_2023_2024.csv -o .\tmp\big5_players_perf.idx -m .\tests\data\big_5_players_stats-schema.yml ^
  --spec "Performance_Gls:desc,Performance_Ast:desc" --limit 1000

rem List columns from a saved schema file
.\target\release\csv-managed.exe schema columns --schema .\tests\data\orders-schema.yml
.\target\release\csv-managed.exe schema columns --schema .\tests\data\big_5_players_stats-schema.yml

rem Probe full file (all rows) to capture mixed column as string
.\target\release\csv-managed.exe schema infer -i .\tests\data\probe_sample_variation.csv -o .\tmp\probe_full-schema.yml --sample-rows 0

rem Inspect a handful of rows and review inferred samples, format hints, and override status directly in the console:
.\target\release\csv-managed.exe schema probe -i tests/data/big_5_players_stats_2023_2024.csv --sample-rows 5

rem Generate a schema file populated with snake_case renames and empty replacement arrays so you can fill in value substitutions later:
.\target\release\csv-managed.exe schema infer --mapping --replace-template -i tests/data/big_5_players_stats_2023_2024.csv -o tmp/big5_inferred-schema.yml --sample-rows 0

rem Preview inferred schema YAML and probe report without writing the output file (dry run)
.\target\release\csv-managed.exe schema infer --mapping --replace-template ^
  -i tests/data/big_5_players_stats_2023_2024.csv -o tmp/big5_preview-schema.yml --sample-rows 25 --preview

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
powershell -NoProfile -Command "$lines = Get-Content .\tests\data\big_5_players_stats_2023_2024.csv | Select-Object -First 25; $text = ($lines -join [Environment]::NewLine) + [Environment]::NewLine; $bytes = [System.Text.Encoding]::GetEncoding(1252).GetBytes($text); [System.IO.File]::WriteAllBytes('.\tmp\big_5_windows1252.csv', $bytes)"

rem Probe using explicit input encoding support
.\target\release\csv-managed.exe schema infer -i .\tmp\big_5_windows1252.csv -o .\tmp\probe_windows-schema.yml --input-encoding windows-1252

.\target\release\csv-managed.exe schema infer -i .\tests\data\datatype_mapping.csv -o .\tmp\datatype_mapping-schema.yml --mapping --replace-template
.\target\release\csv-managed.exe schema infer -i .\tests\data\orders_invalid.csv -o .\tmp\orders-schema.yml --mapping --replace-template
.\target\release\csv-managed.exe schema infer -i .\tests\data\orders_temporal.csv -o .\tmp\orders_temporal-schema.yml --mapping --replace-template
.\target\release\csv-managed.exe schema infer -i .\tests\data\stats_schema.csv -o .\tmp\stats_schema-schema.yml --mapping --replace-template
.\target\release\csv-managed.exe schema infer -i .\tests\data\stats_temporal.csv -o .\tmp\stats_temporal-schema.yml --mapping --replace-template

.\target\release\csv-managed.exe schema infer -i .\tests\data\big_5_players_stats_2023_2024.csv -o .\tmp\big_5_players_stats-schema.yml --mapping --replace-template
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

rem Stats command examples
.\target\release\csv-managed.exe stats -i .\tests\data\stats_infer.csv
.\target\release\csv-managed.exe stats -i .\tests\data\stats_schema.csv -m .\tests\data\stats_schema-schema.yml --columns price
.\target\release\csv-managed.exe stats -i .\tests\data\stats_schema.csv -m .\tests\data\stats_schema-schema.yml --columns quantity --limit 2
.\target\release\csv-managed.exe stats -i .\tests\data\stats_temporal.csv -m .\tests\data\stats_temporal-schema.yml --columns ordered_at --columns ordered_at_ts --columns ship_time
.\target\release\csv-managed.exe stats -i .\tests\data\stats_schema.csv -m .\tests\data\stats_schema-schema.yml --frequency --top 5
.\target\release\csv-managed.exe stats -i .\tests\data\big_5_players_stats_2023_2024.csv --frequency -C Squad --filter "Player=Max Aarons"
