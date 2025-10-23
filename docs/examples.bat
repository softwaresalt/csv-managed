if not exist .\tmp mkdir .\tmp

rem Schema command examples
REM Wrap replacement specifications in quotes so PowerShell/CMD do not treat '>' as redirection
.\target\release\csv-managed.exe schema -o .\tmp\schema_basic.schema -c "id:integer" -c "name:string" -c "amount:float"
::type .\tmp\schema_basic.schema
.\target\release\csv-managed.exe schema -o .\tmp\schema_alias.schema -c "status:string->order_status" -c "created_at:datetime" --replace "status=pending->ready" --replace "status=unknown->ready"
::type .\tmp\schema_alias.schema
.\target\release\csv-managed.exe schema -o .\tmp\schema_list.schema -c "id:integer,status:string,created_at:datetime"
::type .\tmp\schema_list.schema

rem List columns from a saved schema file
.\target\release\csv-managed.exe schema columns --schema .\tests\data\orders.schema
.\target\release\csv-managed.exe schema columns --schema .\tests\data\big_5_players_stats.schema

rem Probe full file (all rows) to capture mixed column as string
.\target\release\csv-managed.exe schema infer -i .\tests\data\probe_sample_variation.csv -o .\tmp\probe_full.schema --sample-rows 0

rem Inspect a handful of rows and review inferred samples, format hints, and override status directly in the console:
.\target\release\csv-managed.exe schema probe -i tests/data/big_5_players_stats_2023_2024.csv --sample-rows 5

rem Generate a schema file populated with snake_case renames and empty replacement arrays so you can fill in value substitutions later:
.\target\release\csv-managed.exe schema infer --mapping --replace-template -i tests/data/big_5_players_stats_2023_2024.csv -o tmp/big5_inferred.schema --sample-rows 0

rem Probe with limited sampling to infer integer type from first row only
.\target\release\csv-managed.exe schema infer -i .\tests\data\probe_sample_variation.csv -o .\tmp\probe_sampled.schema --sample-rows 1

rem This command performs a full scan (`--sample-rows 0`) before writing `tmp/big5_inferred.schema`.
.\target\release\csv-managed.exe schema infer --mapping --override Performance_Gls:integer --override "Per 90 Minutes_Gls:string" ^
  -i tests/data/big_5_players_stats_2023_2024.csv -o tmp/big5_overrides.schema --sample-rows 10

rem Capture the current probe table rendering and fail future runs if the layout changes unexpectedly:
.\target\release\csv-managed.exe schema probe -i tests/data/big_5_players_stats_2023_2024.csv --sample-rows 5 --snapshot tmp/big5_probe.snap

rem Create a Windows-1252 encoded CSV derived from the Big 5 stats dataset
powershell -NoProfile -Command "$lines = Get-Content .\tests\data\big_5_players_stats_2023_2024.csv | Select-Object -First 25; $text = ($lines -join [Environment]::NewLine) + [Environment]::NewLine; $bytes = [System.Text.Encoding]::GetEncoding(1252).GetBytes($text); [System.IO.File]::WriteAllBytes('.\tmp\big_5_windows1252.csv', $bytes)"

rem Probe using explicit input encoding support
.\target\release\csv-managed.exe schema infer -i .\tmp\big_5_windows1252.csv -o .\tmp\probe_windows.schema --input-encoding windows-1252

.\target\release\csv-managed.exe schema infer -i .\tests\data\big_5_players_stats_2023_2024.csv -o .\tests\data\big_5_players_stats.schema --mapping --replace-template
.\target\release\csv-managed.exe schema verify -m .\tests\data\big_5_players_stats.schema -i .\tests\data\big_5_players_stats_2023_2024.csv

.\target\debug\csv-managed.exe schema verify -m .\tests\data\orders.schema -i .\tests\data\orders_invalid.csv --report-invalid
.\target\debug\csv-managed.exe schema verify -m .\tests\data\orders.schema -i .\tests\data\orders_invalid.csv --report-invalid:detail 5
.\target\debug\csv-managed.exe schema verify -m .\tests\data\orders.schema -i .\tests\data\orders_invalid.csv --report-invalid:detail:summary

rem Prepare preview subsets derived from the Big 5 stats dataset
.\target\release\csv-managed.exe process -i .\tests\data\big_5_players_stats_2023_2024.csv --limit 15 --columns Rank --columns Player --columns Squad -o .\tmp\big_5_preview.csv
.\target\release\csv-managed.exe process -i .\tests\data\big_5_players_stats_2023_2024.csv --limit 5 --columns Rank --columns Player --columns Squad --output-delimiter tab -o .\tmp\big_5_preview.tsv
.\target\release\csv-managed.exe process -i .\tests\data\big_5_players_stats_2023_2024.csv --limit 5 --columns Rank --columns Player --columns Squad --output-delimiter "|" -o .\tmp\big_5_preview_pipe.csv

rem Preview default row count (10 rows) from the primary dataset
.\target\release\csv-managed.exe preview -i .\tests\data\big_5_players_stats_2023_2024.csv

rem Preview with an explicit row limit
.\target\release\csv-managed.exe preview -i .\tests\data\big_5_players_stats_2023_2024.csv --rows 5

rem Preview auto-detects tab-delimited files via extension
.\target\release\csv-managed.exe preview -i .\tmp\big_5_preview.tsv

rem Preview with a custom delimiter override (pipe-separated values)
.\target\release\csv-managed.exe preview -i .\tmp\big_5_preview_pipe.csv --delimiter "|"

rem Preview using explicit input encoding for Windows-1252 data
.\target\release\csv-managed.exe preview -i .\tmp\big_5_windows1252.csv --input-encoding windows-1252

rem Stats command examples
.\target\release\csv-managed.exe stats -i .\tests\data\stats_infer.csv
.\target\release\csv-managed.exe stats -i .\tests\data\stats_schema.csv -m .\tests\data\stats_schema.schema --columns price
.\target\release\csv-managed.exe stats -i .\tests\data\stats_schema.csv -m .\tests\data\stats_schema.schema --columns quantity --limit 2
.\target\release\csv-managed.exe stats -i .\tests\data\stats_temporal.csv -m .\tests\data\stats_temporal.schema --columns ordered_at --columns ordered_at_ts --columns ship_time
.\target\release\csv-managed.exe stats -i .\tests\data\stats_schema.csv -m .\tests\data\stats_schema.schema --frequency --top 5
.\target\release\csv-managed.exe stats -i .\tests\data\big_5_players_stats_2023_2024.csv --frequency -C Squad --filter "Player=Max Aarons"
