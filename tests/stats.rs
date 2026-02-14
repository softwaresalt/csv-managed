use std::path::{Path, PathBuf};

use assert_cmd::Command;
use csv::{ReaderBuilder, WriterBuilder};
use predicates::{prelude::PredicateBooleanExt, str::contains};
use tempfile::tempdir;

use csv_managed::schema::{Schema, ValueReplacement};

const BIG5_DATA: &str = "big_5_players_stats_2023_2024.csv";
const GOALS_COLUMN: &str = "Performance_Gls";
const ASSISTS_COLUMN: &str = "Performance_Ast";
const ORDERS_TEMPORAL_DATA: &str = "orders_temporal.csv";
const ORDERS_TEMPORAL_SCHEMA: &str = "orders_temporal-schema.yml";
const ORDERED_AT_COL: &str = "ordered_at";
const ORDERED_AT_TS_COL: &str = "ordered_at_ts";
const SHIP_TIME_COL: &str = "ship_time";
const STATS_TEMPORAL_DATA: &str = "stats_temporal.csv";
const STATS_TEMPORAL_SCHEMA: &str = "stats_temporal-schema.yml";

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("data")
        .join(name)
}

fn write_big5_numeric_subset(
    source: &Path,
    target: &Path,
    rows: usize,
    mutate_second_to_na: bool,
) -> f64 {
    let mut reader = ReaderBuilder::new()
        .has_headers(true)
        .from_path(source)
        .expect("open big5 source");
    let headers = reader.headers().expect("headers").clone();
    let goals_idx = headers
        .iter()
        .position(|h| h == GOALS_COLUMN)
        .expect("goal header index");
    let assists_idx = headers
        .iter()
        .position(|h| h == ASSISTS_COLUMN)
        .expect("assist header index");

    let mut writer = WriterBuilder::new()
        .from_path(target)
        .expect("create subset writer");
    writer
        .write_record([GOALS_COLUMN, ASSISTS_COLUMN])
        .expect("write subset header");

    let mut first_value = None;
    let mut rows_written = 0usize;
    for result in reader.records() {
        let record = result.expect("record");
        let goal_raw = record.get(goals_idx).unwrap_or("").trim();
        let assist_raw = record.get(assists_idx).unwrap_or("").trim();
        if goal_raw.is_empty() || assist_raw.is_empty() {
            continue;
        }

        let goal_numeric = match goal_raw.parse::<f64>() {
            Ok(value) => value,
            Err(_) => continue,
        };
        let assist_numeric = match assist_raw.parse::<f64>() {
            Ok(value) => value,
            Err(_) => continue,
        };

        if rows_written == 0 {
            first_value = Some(goal_numeric);
        }

        let goal_value = if mutate_second_to_na && rows_written == 1 {
            "NA".to_string()
        } else {
            goal_numeric.to_string()
        };
        let assist_value = assist_numeric.to_string();
        writer
            .write_record([goal_value, assist_value])
            .expect("write subset row");

        rows_written += 1;
        if rows_written >= rows {
            break;
        }
    }

    writer.flush().expect("flush subset writer");
    assert_eq!(
        rows_written, rows,
        "expected to write {rows} rows but wrote {rows_written}"
    );
    first_value.expect("captured first goal value")
}

#[test]
fn stats_infers_numeric_columns_from_big5() {
    let csv_path = fixture_path(BIG5_DATA);
    let temp = tempdir().expect("temp dir");
    let subset_path = temp.path().join("big5_numeric.csv");
    write_big5_numeric_subset(&csv_path, &subset_path, 5, false);
    Command::cargo_bin("csv-managed")
        .expect("binary exists")
        .args([
            "stats",
            "-i",
            subset_path.to_str().unwrap(),
            "--limit",
            "50",
        ])
        .assert()
        .success()
        .stdout(
            contains("column")
                .and(contains(GOALS_COLUMN))
                .and(contains(ASSISTS_COLUMN))
                .and(contains("mean")),
        );
}

#[test]
fn stats_columns_flag_limits_output_with_big5_schema() {
    let csv_path = fixture_path(BIG5_DATA);
    let temp = tempdir().expect("temp dir");
    let subset_path = temp.path().join("big5_numeric.csv");
    write_big5_numeric_subset(&csv_path, &subset_path, 5, false);
    let schema_path = temp.path().join("big5-schema.yml");

    Command::cargo_bin("csv-managed")
        .expect("binary exists")
        .args([
            "schema",
            "infer",
            "-i",
            subset_path.to_str().unwrap(),
            "-o",
            schema_path.to_str().unwrap(),
            "--sample-rows",
            "0",
        ])
        .assert()
        .success();

    let assert = Command::cargo_bin("csv-managed")
        .expect("binary exists")
        .args([
            "stats",
            "-i",
            subset_path.to_str().unwrap(),
            "-m",
            schema_path.to_str().unwrap(),
            "--columns",
            GOALS_COLUMN,
            "--limit",
            "100",
        ])
        .assert()
        .success();

    let stdout = String::from_utf8(assert.get_output().stdout.clone()).expect("stdout utf8");
    assert!(
        stdout.contains(GOALS_COLUMN),
        "goal column missing: {stdout}"
    );
    assert!(
        !stdout.contains(ASSISTS_COLUMN),
        "assist column unexpectedly present: {stdout}"
    );
}

#[test]
fn stats_applies_replacements_and_limit_on_big5_subset() {
    let csv_path = fixture_path(BIG5_DATA);
    let temp = tempdir().expect("temp dir");
    let clean_subset_path = temp.path().join("big5_clean.csv");
    let subset_path = temp.path().join("big5_subset.csv");
    let schema_path = temp.path().join("big5-schema.yml");

    write_big5_numeric_subset(&csv_path, &clean_subset_path, 3, false);
    Command::cargo_bin("csv-managed")
        .expect("binary exists")
        .args([
            "schema",
            "infer",
            "-i",
            clean_subset_path.to_str().unwrap(),
            "-o",
            schema_path.to_str().unwrap(),
            "--sample-rows",
            "0",
        ])
        .assert()
        .success();

    let mut schema = Schema::load(&schema_path).expect("load schema");
    let goals_idx = schema
        .column_index(GOALS_COLUMN)
        .expect("goals column index");
    schema.columns[goals_idx]
        .value_replacements
        .push(ValueReplacement {
            from: "NA".to_string(),
            to: "0".to_string(),
        });
    schema
        .save(&schema_path)
        .expect("save schema with replacement");

    let original_value = write_big5_numeric_subset(&csv_path, &subset_path, 3, true);
    let expected_mean = (original_value + 0.0) / 2.0;
    let expected_mean_str = if expected_mean.fract() == 0.0 {
        format!("{expected_mean:.0}")
    } else {
        format!("{expected_mean:.4}")
    };

    let assert = Command::cargo_bin("csv-managed")
        .expect("binary exists")
        .args([
            "stats",
            "-i",
            subset_path.to_str().unwrap(),
            "-m",
            schema_path.to_str().unwrap(),
            "--columns",
            GOALS_COLUMN,
            "--limit",
            "2",
        ])
        .assert()
        .success();

    let stdout = String::from_utf8(assert.get_output().stdout.clone()).expect("stdout utf8");
    let stats_line = stdout
        .lines()
        .find(|line| line.contains(GOALS_COLUMN))
        .expect("stats output line");
    let columns = parse_table_row(stats_line);

    assert!(
        columns.len() >= 7,
        "unexpected column formatting: {columns:?}"
    );
    assert_eq!(columns[1], "2", "limit should restrict to two rows");
    assert_eq!(
        columns[4], expected_mean_str,
        "mean should reflect replacement"
    );
}

#[test]
fn stats_frequency_reports_categorical_counts() {
    let csv_path = fixture_path("stats_schema.csv");
    let schema_path = fixture_path("stats_schema-schema.yml");
    let assert = Command::cargo_bin("csv-managed")
        .expect("binary exists")
        .args([
            "stats",
            "-i",
            csv_path.to_str().unwrap(),
            "-m",
            schema_path.to_str().unwrap(),
            "--frequency",
            "-C",
            "status",
            "--top",
            "5",
        ])
        .assert()
        .success();

    let stdout = String::from_utf8(assert.get_output().stdout.clone()).expect("stdout utf8");
    assert!(stdout.contains("status"), "status column missing: {stdout}");
    assert!(stdout.contains("good"), "expected 'good' frequency");
    assert!(
        stdout.contains("backorder"),
        "expected 'backorder' frequency"
    );
    assert!(stdout.contains("count"), "missing count header: {stdout}");
}

#[test]
fn stats_filter_limits_rows_for_summary() {
    let csv_path = fixture_path(BIG5_DATA);
    let temp = tempdir().expect("temp dir");
    let subset_path = temp.path().join("big5_filtered.csv");
    write_big5_numeric_subset(&csv_path, &subset_path, 5, false);

    let assert = Command::cargo_bin("csv-managed")
        .expect("binary exists")
        .args([
            "stats",
            "-i",
            subset_path.to_str().unwrap(),
            "--columns",
            GOALS_COLUMN,
            "--filter",
            &format!("{ASSISTS_COLUMN}>=1"),
        ])
        .assert()
        .success();

    let stdout = String::from_utf8(assert.get_output().stdout.clone()).expect("stdout utf8");
    let row = stdout
        .lines()
        .find(|line| line.contains(GOALS_COLUMN))
        .expect("goals row");
    let cells = parse_table_row(row);
    assert_eq!(cells[0], GOALS_COLUMN);
    assert_eq!(cells[1], "3", "filter should restrict to assists >= 1");
    assert_eq!(cells[4], "0.6667", "mean should reflect filtered goals");
}

#[test]
fn stats_frequency_honors_filters() {
    let data_path = fixture_path(BIG5_DATA);

    let assert = Command::cargo_bin("csv-managed")
        .expect("binary exists")
        .args([
            "stats",
            "-i",
            data_path.to_str().unwrap(),
            "--frequency",
            "-C",
            "Squad",
            "--filter",
            "Player=Max Aarons",
        ])
        .assert()
        .success();

    let stdout = String::from_utf8(assert.get_output().stdout.clone()).expect("stdout utf8");
    assert!(
        stdout.contains("Bournemouth"),
        "expected squad to remain: {stdout}"
    );
    assert!(
        !stdout.contains("Union Berlin"),
        "unexpected squad found after filtering: {stdout}"
    );
}

fn parse_table_row(line: &str) -> Vec<String> {
    let mut cells = Vec::new();
    let mut current = String::new();
    let mut space_run = 0usize;

    for ch in line.chars() {
        if ch == ' ' {
            space_run += 1;
            continue;
        }

        if space_run >= 2 {
            if !current.trim().is_empty() {
                cells.push(current.trim().to_string());
            }
            current.clear();
        } else if space_run == 1 && !current.is_empty() {
            current.push(' ');
        }

        space_run = 0;
        current.push(ch);
    }

    if space_run >= 2 {
        if !current.trim().is_empty() {
            cells.push(current.trim().to_string());
        }
        current.clear();
    } else if space_run == 1 {
        current.push(' ');
    }

    if !current.trim().is_empty() {
        cells.push(current.trim().to_string());
    }

    cells
}

#[test]
fn stats_handles_temporal_columns_from_schema() {
    let data_path = fixture_path(ORDERS_TEMPORAL_DATA);
    let schema_path = fixture_path(ORDERS_TEMPORAL_SCHEMA);

    let assert = Command::cargo_bin("csv-managed")
        .expect("binary exists")
        .args([
            "stats",
            "-i",
            data_path.to_str().unwrap(),
            "-m",
            schema_path.to_str().unwrap(),
            "--columns",
            ORDERED_AT_COL,
            "--columns",
            ORDERED_AT_TS_COL,
            "--columns",
            SHIP_TIME_COL,
        ])
        .assert()
        .success();

    let stdout = String::from_utf8(assert.get_output().stdout.clone()).expect("stdout utf8");

    let ordered_line = stdout
        .lines()
        .find(|line| line.contains(ORDERED_AT_COL))
        .expect("ordered_at row present");
    let ordered_cells = parse_table_row(ordered_line);
    assert_eq!(ordered_cells[0], ORDERED_AT_COL);
    assert_eq!(ordered_cells[1], "4");
    assert_eq!(ordered_cells[2], "2024-01-01");
    assert_eq!(ordered_cells[3], "2024-02-10");
    assert!(
        ordered_cells[6].ends_with("days"),
        "std dev should note days"
    );

    let ordered_ts_line = stdout
        .lines()
        .find(|line| line.contains(ORDERED_AT_TS_COL))
        .expect("ordered_at_ts row present");
    let ordered_ts_cells = parse_table_row(ordered_ts_line);
    assert_eq!(ordered_ts_cells[0], ORDERED_AT_TS_COL);
    assert_eq!(ordered_ts_cells[1], "4");
    assert_eq!(ordered_ts_cells[2], "2024-01-01 06:00:00");
    assert_eq!(ordered_ts_cells[3], "2024-02-10 14:00:00");
    assert!(
        ordered_ts_cells[6].ends_with("seconds"),
        "std dev should note seconds"
    );

    let ship_time_line = stdout
        .lines()
        .find(|line| line.contains(SHIP_TIME_COL))
        .expect("ship_time row present");
    let ship_time_cells = parse_table_row(ship_time_line);
    assert_eq!(ship_time_cells[0], SHIP_TIME_COL);
    assert_eq!(ship_time_cells[1], "4");
    assert_eq!(ship_time_cells[2], "06:00:00");
    assert_eq!(ship_time_cells[3], "16:30:00");
    assert_eq!(ship_time_cells[4], "09:37:30");
    assert_eq!(ship_time_cells[5], "08:00:00");
    assert!(
        ship_time_cells[6].ends_with("seconds"),
        "time std dev should note seconds"
    );
}

#[test]
fn stats_includes_temporal_columns_by_default() {
    let data_path = fixture_path(STATS_TEMPORAL_DATA);
    let schema_path = fixture_path(STATS_TEMPORAL_SCHEMA);

    let assert = Command::cargo_bin("csv-managed")
        .expect("binary exists")
        .args([
            "stats",
            "-i",
            data_path.to_str().unwrap(),
            "-m",
            schema_path.to_str().unwrap(),
            "--limit",
            "0",
        ])
        .assert()
        .success();

    let stdout = String::from_utf8(assert.get_output().stdout.clone()).expect("stdout utf8");

    assert!(
        stdout.contains(ORDERED_AT_COL),
        "date column missing: {stdout}"
    );
    assert!(
        stdout.contains(ORDERED_AT_TS_COL),
        "datetime column missing: {stdout}"
    );
    assert!(
        stdout.contains(SHIP_TIME_COL),
        "time column missing: {stdout}"
    );
    assert!(stdout.contains("id"), "integer column missing: {stdout}");
    assert!(
        !stdout.contains("status"),
        "string column should not be present: {stdout}"
    );
}

#[test]
fn stats_preserves_currency_precision_in_output() {
    let data_path = fixture_path("currency_transactions.csv");
    let schema_path = fixture_path("currency_transactions-schema.yml");

    let assert = Command::cargo_bin("csv-managed")
        .expect("binary exists")
        .args([
            "stats",
            "-i",
            data_path.to_str().unwrap(),
            "-m",
            schema_path.to_str().unwrap(),
        ])
        .assert()
        .success();

    let stdout = String::from_utf8(assert.get_output().stdout.clone()).expect("stdout utf8");

    // gross_amount_raw is Currency with scale 2 (round)
    let gross_line = stdout
        .lines()
        .find(|line| line.contains("gross_amount_raw"))
        .expect("gross_amount_raw row present");
    let gross_cells = parse_table_row(gross_line);
    assert_eq!(gross_cells[0], "gross_amount_raw");
    assert_eq!(gross_cells[1], "3", "should have 3 rows");

    // tax_raw is Currency with scale 4 (truncate) — precision should be preserved
    let tax_line = stdout
        .lines()
        .find(|line| line.contains("tax_raw"))
        .expect("tax_raw row present");
    let tax_cells = parse_table_row(tax_line);
    assert_eq!(tax_cells[0], "tax_raw");
    assert_eq!(tax_cells[1], "3", "should have 3 rows");
    // min should be 0.0000 (scale 4)
    assert_eq!(tax_cells[2], "0.0000", "min should preserve 4-digit scale");

    // rebate_currency should also be present as Currency
    let rebate_line = stdout
        .lines()
        .find(|line| line.contains("rebate_currency"))
        .expect("rebate_currency row present");
    let rebate_cells = parse_table_row(rebate_line);
    assert_eq!(rebate_cells[0], "rebate_currency");
    assert_eq!(rebate_cells[1], "3", "should have 3 rows");
}

#[test]
fn stats_preserves_decimal_precision_in_output() {
    let data_path = fixture_path("decimal_measurements.csv");
    let schema_path = fixture_path("decimal_measurements-schema.yml");

    let assert = Command::cargo_bin("csv-managed")
        .expect("binary exists")
        .args([
            "stats",
            "-i",
            data_path.to_str().unwrap(),
            "-m",
            schema_path.to_str().unwrap(),
        ])
        .assert()
        .success();

    let stdout = String::from_utf8(assert.get_output().stdout.clone()).expect("stdout utf8");

    // measurement_round is decimal(10,2)
    let round_line = stdout
        .lines()
        .find(|line| line.contains("measurement_round"))
        .expect("measurement_round row present");
    let round_cells = parse_table_row(round_line);
    assert_eq!(round_cells[0], "measurement_round");
    assert_eq!(round_cells[1], "4", "should have 4 rows");

    // measurement_truncate is decimal(10,3)
    let trunc_line = stdout
        .lines()
        .find(|line| line.contains("measurement_truncate"))
        .expect("measurement_truncate row present");
    let trunc_cells = parse_table_row(trunc_line);
    assert_eq!(trunc_cells[0], "measurement_truncate");
    assert_eq!(trunc_cells[1], "4", "should have 4 rows");

    // measurement_exact is decimal(12,4) — min should preserve scale
    let exact_line = stdout
        .lines()
        .find(|line| line.contains("measurement_exact"))
        .expect("measurement_exact row present");
    let exact_cells = parse_table_row(exact_line);
    assert_eq!(exact_cells[0], "measurement_exact");
    assert_eq!(exact_cells[1], "4", "should have 4 rows");
    // min is 0.0001, max is 1000.0000 — should preserve 4-digit scale
    assert_eq!(
        exact_cells[2], "0.0001",
        "min should preserve 4-digit scale"
    );
    assert_eq!(
        exact_cells[3], "1000.0000",
        "max should preserve 4-digit scale"
    );
}
