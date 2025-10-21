if not exist .\tmp mkdir .\tmp

rem Probe full file (all rows) to capture mixed column as string
.\target\release\csv-managed.exe probe -i .\tests\data\probe_sample_variation.csv -m .\tmp\probe_full.schema --sample-rows 0

rem Probe with limited sampling to infer integer type from first row only
.\target\release\csv-managed.exe probe -i .\tests\data\probe_sample_variation.csv -m .\tmp\probe_sampled.schema --sample-rows 1

rem Create a Windows-1252 encoded CSV containing non-ASCII characters
powershell -NoProfile -Command "$content = \"id,name`n1,Caf$([char]0x00E9)`n\"; $bytes = [System.Text.Encoding]::GetEncoding(1252).GetBytes($content); [System.IO.File]::WriteAllBytes('.\tmp\probe_windows1252.csv', $bytes)"

rem Probe using explicit input encoding support
.\target\release\csv-managed.exe probe -i .\tmp\probe_windows1252.csv -m .\tmp\probe_windows.schema --input-encoding windows-1252

.\target\release\csv-managed.exe probe -i .\tests\data\big_5_players_stats_2023_2024.csv -m .\tests\data\big_5_players_stats.schema --mapping --replace
.\target\release\csv-managed.exe verify -m .\tests\data\big_5_players_stats.schema -i .\tests\data\big_5_players_stats_2023_2024.csv

rem Stats command examples
.\target\release\csv-managed.exe stats -i .\tests\data\stats_infer.csv
.\target\release\csv-managed.exe stats -i .\tests\data\stats_schema.csv -m .\tests\data\stats_schema.schema --columns price
.\target\release\csv-managed.exe stats -i .\tests\data\stats_schema.csv -m .\tests\data\stats_schema.schema --columns quantity --limit 2

.\target\debug\csv-managed.exe verify -m .\tests\data\orders.schema -i .\tests\data\orders_invalid.csv --report-invalid
.\target\debug\csv-managed.exe verify -m .\tests\data\orders.schema -i .\tests\data\orders_invalid.csv --report-invalid:detail 5
.\target\debug\csv-managed.exe verify -m .\tests\data\orders.schema -i .\tests\data\orders_invalid.csv --report-invalid:detail:summary
