Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$scriptRoot = Split-Path -Path $MyInvocation.MyCommand.Path -Parent
$repoRoot = (Resolve-Path (Join-Path $scriptRoot '..')).Path
$binRelease = Join-Path $repoRoot 'target\release\csv-managed.exe'
$binDebug = Join-Path $repoRoot 'target\debug\csv-managed.exe'
$csvBig5 = Join-Path $repoRoot 'tests\data\big_5_players_stats_2023_2024.csv'
$schemaBig5 = Join-Path $repoRoot 'tests\data\big_5_players_stats-schema.yml'
$statsCsv = Join-Path $repoRoot 'tests\data\stats_schema.csv'
$statsSchema = Join-Path $repoRoot 'tests\data\stats_schema-schema.yml'

Push-Location $repoRoot
try {
    if (-not (Test-Path 'tmp')) {
        New-Item -ItemType Directory -Path 'tmp' | Out-Null
    }

    # Schema command examples
    & "$binRelease" schema -o 'tmp\schema_basic-schema.yml' -c 'id:integer' -c 'name:string' -c 'amount:float'
    # Get-Content 'tmp\schema_basic-schema.yml'
    & "$binRelease" schema -o 'tmp\schema_alias-schema.yml' -c 'status:string->order_status' -c 'created_at:datetime' --replace 'status=pending->ready' --replace 'status=unknown->ready'
    # Get-Content 'tmp\schema_alias-schema.yml'
    & "$binRelease" schema -o 'tmp\schema_list-schema.yml' -c 'id:integer,status:string,created_at:datetime'
    # Get-Content 'tmp\schema_list-schema.yml'

    & "$binRelease" index -i 'tests\data\orders_temporal.csv' -o 'tmp\orders_temporal_covering.idx' -m 'tests\data\orders_temporal-schema.yml' --covering 'orders=ordered_at:asc|desc,status:asc'

    # Demonstrate majority-based inference recovering non-string types from noisy data
    & "$binRelease" schema probe -i 'tests\data\majority_datatypes.csv' --sample-rows 0

    # Probe a headerless CSV; inferred columns use field_# names
    & "$binRelease" schema probe -i 'tests\data\sensor_readings_no_header.csv' --mapping --sample-rows 0

    # Index command examples
    & "$binRelease" index -i 'tests\data\orders_temporal.csv' -o 'tmp\orders_temporal_ordered_at.idx' -m 'tests\data\orders_temporal-schema.yml' -C ordered_at

    & "$binRelease" index -i 'tests\data\orders_temporal.csv' -o 'tmp\orders_temporal_variants.idx' -m 'tests\data\orders_temporal-schema.yml' --spec 'recent=ordered_at:desc' --spec 'ordered_at:asc,ship_time:desc'

    & "$binRelease" process -i 'tests\data\orders_temporal.csv' -m 'tests\data\orders_temporal-schema.yml' -x 'tmp\orders_temporal_variants.idx' --index-variant recent --sort 'ordered_at:desc' --columns ordered_at --columns status --limit 10 --preview

    & "$binRelease" process -i 'tests\data\orders_temporal.csv' -m 'tests\data\orders_temporal-schema.yml' -x 'tmp\orders_temporal_variants.idx' --index-variant recent --sort 'ordered_at:desc' --limit 10 --preview

    & "$binRelease" index -i 'tests\data\orders_temporal.csv' -o 'tmp\orders_temporal_covering.idx' -m 'tests\data\orders_temporal-schema.yml' --covering 'orders=ordered_at:asc|desc,status:asc'

    & "$binRelease" index -i 'tests\data\big_5_players_stats_2023_2024.csv' -o 'tmp\big5_players_perf.idx' -m 'tests\data\big_5_players_stats-schema.yml' --spec 'Performance_Gls:desc,Performance_Ast:desc' --limit 1000

    # List columns from a saved schema file
    & "$binRelease" schema columns --schema 'tests\data\orders-schema.yml'
    & "$binRelease" schema columns --schema 'tests\data\big_5_players_stats-schema.yml'

    & "$binRelease" schema infer -i 'tests\data\probe_sample_variation.csv' -o 'tmp\probe_full-schema.yml' --sample-rows 0

    & "$binRelease" schema probe -i 'tests\data\big_5_players_stats_2023_2024.csv' --sample-rows 0
    & "$binRelease" schema probe -i 'tests\data\big_5_players_stats_2023_2024.csv' --mapping --sample-rows 250

    & "$binRelease" schema infer --mapping --replace-template -i 'tests\data\big_5_players_stats_2023_2024.csv' -o 'tmp\big5_inferred-schema.yml' --sample-rows 0

    & "$binRelease" schema infer --mapping --replace-template -i 'tests\data\big_5_players_stats_2023_2024.csv' -o 'tmp\big5_preview-schema.yml' --sample-rows 0 --preview

    & "$binRelease" schema infer -i 'tests\data\big_5_players_stats_2023_2024.csv' --sample-rows 0 --diff 'tmp\big5_inferred-schema.yml'

    & "$binRelease" schema infer -i 'tests\data\probe_sample_variation.csv' -o 'tmp\probe_sampled-schema.yml' --sample-rows 1

    & "$binRelease" schema infer --mapping --override 'Performance_Gls:integer' --override 'Per 90 Minutes_Gls:string' -i 'tests\data\big_5_players_stats_2023_2024.csv' -o 'tmp\big5_overrides-schema.yml' --sample-rows 10

    & "$binRelease" schema probe -i 'tests\data\big_5_players_stats_2023_2024.csv' --sample-rows 10 --snapshot 'tmp\big5_probe.snap'

    & "$binRelease" schema infer -i 'tests\data\big_5_players_stats_2023_2024.csv' --sample-rows 0 --snapshot 'tmp\big5_probe.snap'

    # Create a Windows-1252 encoded CSV derived from the Big 5 stats dataset
    & "$binRelease" process -i 'tests\data\big_5_players_stats_2023_2024.csv' -m 'tests\data\big_5_players_stats-schema.yml' --limit 25 --output 'tmp\big_5_windows1252.csv' --output-encoding windows-1252

    & "$binRelease" schema infer -i 'tmp\big_5_windows1252.csv' -o 'tmp\probe_windows-schema.yml' --input-encoding windows-1252

    & "$binRelease" schema infer -i 'tests\data\datatype_mapping.csv' -o 'tmp\datatype_mapping-schema.yml' --mapping --replace-template
    & "$binRelease" schema infer -i 'tests\data\orders_invalid.csv' -o 'tmp\orders-schema.yml' --mapping --replace-template
    & "$binRelease" schema infer -i 'tests\data\orders_temporal.csv' -o 'tmp\orders_temporal-schema.yml' --mapping --replace-template
    & "$binRelease" schema infer -i 'tests\data\stats_schema.csv' -o 'tmp\stats_schema-schema.yml' --mapping --replace-template
    & "$binRelease" schema infer -i 'tests\data\stats_temporal.csv' -o 'tmp\stats_temporal-schema.yml' --mapping --replace-template

    & "$binRelease" schema infer -i 'tests\data\big_5_players_stats_2023_2024.csv' -o 'tmp\big_5_players_stats-schema.yml' --mapping --replace-template
    & "$binRelease" schema verify -m 'tests\data\big_5_players_stats-schema.yml' -i 'tests\data\big_5_players_stats_2023_2024.csv'
    & "$binRelease" schema verify -m 'tests\data\orders-schema.yml' -i 'tests\data\orders_invalid.csv' --report-invalid

    # Datatype mapping feature: reuse sample data and schema from tests/data
    & "$binRelease" schema columns --schema 'tests\data\datatype_mapping-schema.yml'
    & "$binRelease" schema verify -m 'tests\data\datatype_mapping-schema.yml' -i 'tests\data\datatype_mapping.csv' --report-invalid

    & "$binRelease" process -i 'tests\data\datatype_mapping.csv' -m 'tests\data\datatype_mapping-schema.yml' --apply-mappings --preview
    & "$binRelease" process -i 'tests\data\datatype_mapping.csv' -m 'tests\data\datatype_mapping-schema.yml' --apply-mappings -o 'tmp\datatype_mapping_clean.csv'
    Get-Content 'tmp\datatype_mapping_clean.csv'

    # Currency datatype coverage: enforce rounding, truncation, and scale validation
    & "$binRelease" schema columns --schema 'tests\data\currency_transactions-schema.yml'
    & "$binRelease" schema verify -m 'tests\data\currency_transactions-schema.yml' -i 'tests\data\currency_transactions.csv'
    & "$binRelease" schema verify -m 'tests\data\currency_transactions-schema.yml' -i 'tests\data\currency_transactions_invalid.csv' --report-invalid
    & "$binRelease" process -i 'tests\data\currency_transactions.csv' -m 'tests\data\currency_transactions-schema.yml' --apply-mappings -o 'tmp\currency_transactions_clean.csv'
    Get-Content 'tmp\currency_transactions_clean.csv'

    # Fixed decimal datatype coverage: configure precision/scale and rounding strategies
    & "$binRelease" schema columns --schema 'tests\data\decimal_measurements-schema.yml'
    & "$binRelease" schema verify -m 'tests\data\decimal_measurements-schema.yml' -i 'tests\data\decimal_measurements.csv' --report-invalid
    & "$binRelease" process -i 'tests\data\decimal_measurements.csv' -m 'tests\data\decimal_measurements-schema.yml' --apply-mappings -o 'tmp\decimal_measurements_clean.csv'
    Get-Content 'tmp\decimal_measurements_clean.csv'

    # Compute statistics and frequency counts on transformed data (mappings run automatically)
    & "$binRelease" stats -i 'tests\data\datatype_mapping.csv' -m 'tests\data\datatype_mapping-schema.yml' --columns amount
    & "$binRelease" stats -i 'tests\data\datatype_mapping.csv' -m 'tests\data\datatype_mapping-schema.yml' --frequency -C status

    # Debug build schema verify examples
    & "$binDebug" schema verify -m 'tests\data\orders-schema.yml' -i 'tests\data\orders_invalid.csv' --report-invalid
    & "$binDebug" schema verify -m 'tests\data\orders-schema.yml' -i 'tests\data\orders_invalid.csv' --report-invalid:detail 5
    & "$binDebug" schema verify -m 'tests\data\orders-schema.yml' -i 'tests\data\orders_invalid.csv' --report-invalid:detail:summary

    # Prepare preview subsets derived from the Big 5 stats dataset
    & "$binRelease" process -i 'tests\data\big_5_players_stats_2023_2024.csv' --limit 15 --columns Rank --columns Player --columns Squad -o 'tmp\big_5_preview.csv'
    & "$binRelease" process -i 'tests\data\big_5_players_stats_2023_2024.csv' --limit 5 --columns Rank --columns Player --columns Squad --output-delimiter tab -o 'tmp\big_5_preview.tsv'
    & "$binRelease" process -i 'tests\data\big_5_players_stats_2023_2024.csv' --limit 5 --columns Rank --columns Player --columns Squad --output-delimiter '|' -o 'tmp\big_5_preview_pipe.csv'

    & "$binRelease" process -i 'tests\data\big_5_players_stats_2023_2024.csv' --preview
    & "$binRelease" process -i 'tests\data\big_5_players_stats_2023_2024.csv' --preview --limit 5
    & "$binRelease" process -i 'tmp\big_5_preview.tsv' --preview
    & "$binRelease" process -i 'tmp\big_5_preview_pipe.csv' --preview --delimiter '|'
    & "$binRelease" process -i 'tmp\big_5_windows1252.csv' --preview --input-encoding windows-1252

    # Process with derived columns, filters, and table output
    & "$binRelease" process -i 'tests\data\sort_types.csv' -m 'tests\data\sort_types-schema.yml' --filter 'bool_col=true' --filter-expr 'float_col>=0.0 and int_col>=0' --derive 'double_int=int_col*2' --exclude-columns guid_col --row-numbers --boolean-format true-false --table --limit 5

    & "$binRelease" process -i 'tests\data\sort_types.csv' -m 'tests\data\sort_types-schema.yml' --filter 'bool_col=false' --columns id --columns bool_col --columns currency_col --columns decimal_col --boolean-format one-zero --output 'tmp\sort_types_filtered.csv' --output-encoding windows-1252
    Get-Content 'tmp\sort_types_filtered.csv'

    # String transform derive example
    & "$binRelease" process -i 'tests\data\big_5_players_stats_2023_2024.csv' -m 'tests\data\big_5_players_stats-schema.yml' `
        --derive 'slug=snake_case(Player)' `
        --derive 'camel_name=camel_case(Player)' `
        --columns Player --limit 5 --table

    # Streaming derive + schema evolution emission example (process -> stats)
    $derivedSchema = Join-Path $repoRoot 'tmp\stats_with_extra-schema.yml'
    $derivedEvolution = Join-Path $repoRoot 'tmp\stats_with_extra-schema.evo.yml'
    Get-Content -Raw $statsCsv |
        & "$binRelease" process -i - --schema $statsSchema `
                --derive double_price:Float=price*2 `
            --emit-schema $derivedSchema `
            --emit-evolution-base $statsSchema `
        | & "$binRelease" stats -i - --schema $derivedSchema -C double_price
    Get-Content $derivedEvolution

    # Prepare subsets for append command demonstrations
    & "$binRelease" process -i 'tests\data\orders_temporal.csv' -m 'tests\data\orders_temporal-schema.yml' --columns id --columns ordered_at --columns status --filter 'status=shipped' -o 'tmp\orders_shipped.csv'
    & "$binRelease" process -i 'tests\data\orders_temporal.csv' -m 'tests\data\orders_temporal-schema.yml' --columns id --columns ordered_at --columns status --filter 'status=cancelled' -o 'tmp\orders_cancelled.csv'

    & "$binRelease" append -i 'tmp\orders_shipped.csv' -i 'tmp\orders_cancelled.csv' --schema 'tests\data\orders_temporal-schema.yml' --output 'tmp\orders_combined.csv'
    Get-Content 'tmp\orders_combined.csv'

    & "$binRelease" schema verify -m 'tests\data\orders_temporal-schema.yml' -i 'tmp\orders_shipped.csv' -i 'tmp\orders_cancelled.csv' -i 'tmp\orders_combined.csv' --report-invalid:summary

    # Stats command examples
    & "$binRelease" stats -i 'tests\data\stats_infer.csv'
    & "$binRelease" stats -i 'tests\data\stats_schema.csv' -m 'tmp\stats_schema-schema.yml' --columns price
    & "$binRelease" stats -i 'tests\data\stats_schema.csv' -m 'tmp\stats_schema-schema.yml' --columns quantity --limit 2
    & "$binRelease" stats -i 'tests\data\stats_temporal.csv' -m 'tmp\stats_temporal-schema.yml' --columns ordered_at --columns ordered_at_ts --columns ship_time
    & "$binRelease" stats -i 'tests\data\stats_schema.csv' -m 'tmp\stats_schema-schema.yml' --frequency --top 5
    & "$binRelease" stats -i 'tests\data\big_5_players_stats_2023_2024.csv' --frequency -C Squad --filter 'Player=Max Aarons'
    & "$binRelease" stats -i 'tests\data\sort_types.csv' -m 'tests\data\sort_types-schema.yml' --filter 'bool_col=true' --filter-expr 'float_col>=0.0' --limit 10

    # Streaming & Pipelines (stdin '-') Examples
    Get-Content -Raw $csvBig5 | & "$binRelease" process -i - --schema $schemaBig5 --columns Player --columns Performance_Gls --limit 5 --table

    Get-Content -Raw $csvBig5 |
        & "$binRelease" process -i - --schema $schemaBig5 --filter 'Performance_Gls>=10' --limit 40 |
        & "$binRelease" stats -i - --schema $schemaBig5 -C Performance_Gls

    Get-Content -Raw $csvBig5 | & "$binRelease" append -i - -i 'tmp\big_5_preview.csv' --schema $schemaBig5 --output 'tmp\players_union.csv'

    Get-Content -Raw $csvBig5 |
        & "$binRelease" process -i - --schema $schemaBig5 --filter 'Performance_Gls>=5' |
        & "$binRelease" stats -i - --schema $schemaBig5 -C Performance_Gls

    Get-Content -Raw $csvBig5 | & "$binRelease" process -i - --columns Player --limit 3 --table

    # Encoding normalization pipeline (Windows-1252 -> UTF-8 projection)
    Get-Content -Raw 'tmp\big_5_windows1252.csv' | & "$binRelease" process -i - --input-encoding windows-1252 --schema $schemaBig5 --columns Player --columns Squad --limit 5 --table
}
finally {
    Pop-Location
}
