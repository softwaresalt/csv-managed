use std::path::{Path, PathBuf};

use assert_cmd::Command;
use csv::{ReaderBuilder, StringRecord, WriterBuilder};
use csv_managed::{
    data::parse_typed_value,
    io_utils,
    schema::{ColumnMeta, ColumnType, Schema, ValueReplacement},
};
use predicates::{prelude::PredicateBooleanExt, str::contains};
use tempfile::tempdir;

const DATA_FILE: &str = "big_5_players_stats_2023_2024.csv";
const PLAYER_COL: &str = "Player";
const GOALS_COL: &str = "Performance_Gls";
const ASSISTS_COL: &str = "Performance_Ast";
const MINUTES_COL: &str = "Playing Time_Min";
const BOOLEAN_COL: &str = "Has_Goals";
const FIRST_PLAYER: &str = "Max Aarons";

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("data")
        .join(name)
}

fn primary_dataset() -> PathBuf {
    fixture_path(DATA_FILE)
}

fn delimiter_for(path: &Path) -> u8 {
    io_utils::resolve_input_delimiter(path, None)
}

fn delimiter_arg(delimiter: u8) -> Option<String> {
    match delimiter {
        b',' => None,
        b'\t' => Some("tab".to_string()),
        other => Some((other as char).to_string()),
    }
}

fn create_schema(dir: &tempfile::TempDir, input: &Path) -> PathBuf {
    create_schema_internal(dir, input, &[])
}

fn create_schema_with_overrides(
    dir: &tempfile::TempDir,
    input: &Path,
    overrides: &[(&str, ColumnType)],
) -> PathBuf {
    create_schema_internal(dir, input, overrides)
}

fn create_schema_internal(
    dir: &tempfile::TempDir,
    input: &Path,
    overrides: &[(&str, ColumnType)],
) -> PathBuf {
    let schema = dir.path().join("schema.schema");
    let input_str = input.to_str().expect("input path utf-8");
    let schema_str = schema.to_str().expect("schema path utf-8");
    let delimiter = delimiter_for(input);
    let mut args = vec![
        "probe".to_string(),
        "-i".to_string(),
        input_str.to_string(),
        "-m".to_string(),
        schema_str.to_string(),
        "--sample-rows".to_string(),
        "0".to_string(),
    ];
    if let Some(value) = delimiter_arg(delimiter) {
        args.push("--delimiter".to_string());
        args.push(value);
    }
    Command::cargo_bin("csv-managed")
        .expect("binary exists")
        .args(&args)
        .assert()
        .success();
    if !overrides.is_empty() {
        let mut schema_doc = Schema::load(&schema).expect("load schema for overrides");
        for (name, ty) in overrides {
            if let Some(column) = schema_doc.columns.iter_mut().find(|col| col.name == *name) {
                column.datatype = ty.clone();
            }
        }
        schema_doc
            .save(&schema)
            .expect("write schema with overrides");
    }
    schema
}

#[derive(Clone, Copy)]
enum ColumnCheck {
    Integer,
}

impl ColumnCheck {
    fn as_column_type(self) -> ColumnType {
        match self {
            ColumnCheck::Integer => ColumnType::Integer,
        }
    }
}

fn create_subset_with_checks(
    dir: &tempfile::TempDir,
    input: &Path,
    checks: &[(&str, ColumnCheck)],
    limit: usize,
) -> PathBuf {
    let delimiter = delimiter_for(input);
    let subset = dir.path().join(if delimiter == b'\t' {
        "subset.tsv"
    } else {
        "subset.csv"
    });
    let mut reader = ReaderBuilder::new()
        .delimiter(delimiter)
        .has_headers(true)
        .from_path(input)
        .expect("open source for subset");
    let headers = reader.headers().expect("headers").clone();
    let mut writer = WriterBuilder::new()
        .delimiter(delimiter)
        .from_path(&subset)
        .expect("subset writer");

    writer
        .write_record(headers.iter())
        .expect("write subset headers");

    let column_checks: Vec<(usize, ColumnCheck)> = checks
        .iter()
        .map(|(name, check)| {
            let idx = headers
                .iter()
                .position(|h| h == *name)
                .unwrap_or_else(|| panic!("missing column {name}"));
            (idx, *check)
        })
        .collect();

    let mut written = 0usize;
    for result in reader.records() {
        let record = result.expect("record");
        let mut valid = true;
        for (idx, check) in &column_checks {
            let value = record.get(*idx).unwrap_or("");
            match parse_typed_value(value, &check.as_column_type()) {
                Ok(Some(_)) => {}
                Ok(None) | Err(_) => {
                    valid = false;
                    break;
                }
            }
        }
        if valid {
            writer
                .write_record(record.iter())
                .expect("write subset row");
            written += 1;
            if written >= limit {
                break;
            }
        }
    }

    writer.flush().expect("flush subset writer");
    assert!(written > 0, "no rows satisfied column checks");
    subset
}

fn count_rows(path: &Path) -> usize {
    let delimiter = delimiter_for(path);
    let mut reader = ReaderBuilder::new()
        .delimiter(delimiter)
        .has_headers(true)
        .from_path(path)
        .expect("open csv for counting");
    reader.records().count()
}

fn read_csv(path: &Path) -> (StringRecord, Vec<StringRecord>) {
    let delimiter = delimiter_for(path);
    let mut reader = ReaderBuilder::new()
        .delimiter(delimiter)
        .has_headers(true)
        .from_path(path)
        .expect("open csv for reading");
    let headers = reader.headers().expect("headers").clone();
    let rows = reader
        .records()
        .map(|r| r.expect("read record"))
        .collect::<Vec<_>>();
    (headers, rows)
}

fn create_boolean_subset(
    dir: &tempfile::TempDir,
    input: &Path,
    limit: usize,
) -> (PathBuf, PathBuf) {
    let path = dir.path().join("boolean_subset.csv");
    let schema_path = dir.path().join("boolean_subset.schema");
    let mut reader = ReaderBuilder::new()
        .has_headers(true)
        .from_path(input)
        .expect("open source for boolean subset");
    let headers = reader.headers().expect("headers").clone();
    let player_idx = headers
        .iter()
        .position(|h| h == PLAYER_COL)
        .expect("player column");
    let goals_idx = headers
        .iter()
        .position(|h| h == GOALS_COL)
        .expect("goals column");
    let mut writer = WriterBuilder::new()
        .from_path(&path)
        .expect("create boolean subset");
    writer
        .write_record([PLAYER_COL, GOALS_COL, BOOLEAN_COL])
        .expect("write boolean headers");

    let mut written = 0usize;
    for result in reader.records() {
        let record = result.expect("record");
        let player = record.get(player_idx).unwrap_or("");
        if player.is_empty() {
            continue;
        }
        let goals_raw = record.get(goals_idx).unwrap_or("0");
        let goals_value = match goals_raw.parse::<f64>() {
            Ok(value) => value,
            Err(_) => continue,
        };
        let has_goals = if goals_value > 0.0 { "true" } else { "false" };
        writer
            .write_record([
                player.to_string(),
                goals_raw.to_string(),
                has_goals.to_string(),
            ])
            .expect("write boolean row");
        written += 1;
        if written >= limit {
            break;
        }
    }

    writer.flush().expect("flush boolean subset");
    assert!(written > 0, "boolean subset had zero rows");

    let schema = Schema {
        columns: vec![
            ColumnMeta {
                name: PLAYER_COL.to_string(),
                datatype: ColumnType::String,
                rename: None,
                value_replacements: Vec::new(),
            },
            ColumnMeta {
                name: GOALS_COL.to_string(),
                datatype: ColumnType::Integer,
                rename: None,
                value_replacements: Vec::new(),
            },
            ColumnMeta {
                name: BOOLEAN_COL.to_string(),
                datatype: ColumnType::Boolean,
                rename: None,
                value_replacements: Vec::new(),
            },
        ],
    };
    schema.save(&schema_path).expect("write boolean schema");

    (path, schema_path)
}

#[test]
fn probe_infers_expected_types_for_big5() {
    let temp = tempdir().expect("tempdir");
    let input = primary_dataset();
    let cleaned =
        create_subset_with_checks(&temp, &input, &[(GOALS_COL, ColumnCheck::Integer)], 1000);
    let schema_path = create_schema(&temp, &cleaned);

    let schema = Schema::load(&schema_path).expect("load schema");
    let find_type = |name: &str| -> ColumnType {
        schema
            .columns
            .iter()
            .find(|column| column.name == name)
            .map(|column| column.datatype.clone())
            .expect("column present")
    };

    assert_eq!(find_type(PLAYER_COL), ColumnType::String);
    assert_eq!(find_type(GOALS_COL), ColumnType::Integer);
    assert_eq!(find_type("Per 90 Minutes_Gls"), ColumnType::Float);
    assert!(schema.columns.len() > 30);
    let float_columns = schema
        .columns
        .iter()
        .filter(|column| matches!(column.datatype, ColumnType::Float))
        .count();
    assert!(float_columns > 0);
}

#[test]
fn process_with_index_respects_sort_order() {
    let temp = tempdir().expect("tempdir");
    let input = primary_dataset();
    let data = create_subset_with_checks(
        &temp,
        &input,
        &[
            (GOALS_COL, ColumnCheck::Integer),
            (ASSISTS_COL, ColumnCheck::Integer),
        ],
        500,
    );
    let schema_path = create_schema_with_overrides(
        &temp,
        &data,
        &[
            (GOALS_COL, ColumnType::Integer),
            (ASSISTS_COL, ColumnType::Integer),
        ],
    );
    let index_path = temp.path().join("data.idx");
    let output_path = temp.path().join("sorted.csv");

    let spec = format!("{GOALS_COL}:desc,{ASSISTS_COL}:asc");
    Command::cargo_bin("csv-managed")
        .expect("binary exists")
        .args([
            "index",
            "-i",
            data.to_str().unwrap(),
            "-o",
            index_path.to_str().unwrap(),
            "--schema",
            schema_path.to_str().unwrap(),
            "--spec",
            spec.as_str(),
        ])
        .assert()
        .success();

    let sort_primary = format!("{GOALS_COL}:desc");
    let sort_secondary = format!("{ASSISTS_COL}:asc");
    Command::cargo_bin("csv-managed")
        .expect("binary exists")
        .args([
            "process",
            "-i",
            data.to_str().unwrap(),
            "-o",
            output_path.to_str().unwrap(),
            "--schema",
            schema_path.to_str().unwrap(),
            "--index",
            index_path.to_str().unwrap(),
            "--sort",
            sort_primary.as_str(),
            "--sort",
            sort_secondary.as_str(),
            "--columns",
            PLAYER_COL,
            "--columns",
            GOALS_COL,
            "--columns",
            ASSISTS_COL,
            "--row-numbers",
            "--limit",
            "25",
        ])
        .assert()
        .success();

    let (headers, rows) = read_csv(&output_path);
    let row_number_idx = headers
        .iter()
        .position(|h| h == "row_number")
        .expect("row number header");
    let goals_idx = headers
        .iter()
        .position(|h| h == GOALS_COL)
        .expect("goals header");

    let mut goal_totals = Vec::new();
    for (idx, record) in rows.iter().enumerate() {
        let row_number: usize = record
            .get(row_number_idx)
            .expect("row number value")
            .parse()
            .expect("row number parse");
        assert_eq!(row_number, idx + 1);
        let goals: i64 = record
            .get(goals_idx)
            .expect("goal value")
            .parse()
            .expect("goals parse");
        goal_totals.push(goals);
    }
    assert!(goal_totals.windows(2).all(|pair| pair[0] >= pair[1]));
}

#[test]
fn process_filters_and_derives_top_scorers() {
    let temp = tempdir().expect("tempdir");
    let input = primary_dataset();
    let data = create_subset_with_checks(&temp, &input, &[(GOALS_COL, ColumnCheck::Integer)], 500);
    let schema_path = create_schema_with_overrides(
        &temp,
        &data,
        &[
            (GOALS_COL, ColumnType::Integer),
            (MINUTES_COL, ColumnType::Integer),
        ],
    );
    let output_path = temp.path().join("filtered.csv");

    let filter_expr = format!("{GOALS_COL} >= 10");
    let derive_expr = "top_scorer=performance_gls >= 10";
    Command::cargo_bin("csv-managed")
        .expect("binary exists")
        .args([
            "process",
            "-i",
            data.to_str().unwrap(),
            "-o",
            output_path.to_str().unwrap(),
            "--schema",
            schema_path.to_str().unwrap(),
            "--filter",
            filter_expr.as_str(),
            "--derive",
            derive_expr,
            "--columns",
            PLAYER_COL,
            "--columns",
            GOALS_COL,
            "--limit",
            "10",
        ])
        .assert()
        .success();

    let (headers, rows) = read_csv(&output_path);
    let goals_idx = headers
        .iter()
        .position(|h| h == GOALS_COL)
        .expect("goals header");
    let flag_idx = headers
        .iter()
        .position(|h| h == "top_scorer")
        .expect("flag header");

    assert!(!rows.is_empty());
    for record in rows {
        let goals: i64 = record
            .get(goals_idx)
            .expect("goal value")
            .parse()
            .expect("goals parse");
        assert!(goals >= 10);
        assert_eq!(record.get(flag_idx).expect("flag value"), "true");
    }
}

#[test]
fn append_merges_player_datasets() {
    let temp = tempdir().expect("tempdir");
    let input = primary_dataset();
    let output_path = temp.path().join("appended.csv");

    let subset_path = temp.path().join("subset.csv");
    {
        let mut reader = ReaderBuilder::new()
            .has_headers(true)
            .from_path(&input)
            .expect("open source for subset");
        let headers = reader.headers().expect("headers").clone();
        let mut writer = WriterBuilder::new()
            .from_path(&subset_path)
            .expect("subset writer");
        writer.write_record(headers.iter()).expect("subset headers");
        for record in reader.records().take(200) {
            let record = record.expect("record");
            let values: Vec<String> = record.iter().map(|v| v.to_string()).collect();
            writer.write_record(&values).expect("subset row");
        }
        writer.flush().expect("flush subset");
    }

    let expected_rows = count_rows(&input) + count_rows(&subset_path);

    Command::cargo_bin("csv-managed")
        .expect("binary exists")
        .args([
            "append",
            "-i",
            input.to_str().unwrap(),
            "-i",
            subset_path.to_str().unwrap(),
            "-o",
            output_path.to_str().unwrap(),
        ])
        .assert()
        .success();

    let actual_rows = count_rows(&output_path);
    assert_eq!(actual_rows, expected_rows);
}

#[test]
fn append_rejects_mismatched_headers() {
    let temp = tempdir().expect("tempdir");
    let input = primary_dataset();
    let mismatch_path = temp.path().join("mismatch.csv");

    let mut reader = ReaderBuilder::new()
        .has_headers(true)
        .from_path(&input)
        .expect("open source for mismatch");
    let mut headers: Vec<String> = reader
        .headers()
        .expect("headers")
        .iter()
        .map(|h| h.to_string())
        .collect();
    headers[0] = "DifferentRank".to_string();
    let mut writer = WriterBuilder::new()
        .from_path(&mismatch_path)
        .expect("mismatch writer");
    writer.write_record(&headers).expect("write headers");
    for record in reader.records().take(50) {
        let record = record.expect("record");
        let values: Vec<String> = record.iter().map(|v| v.to_string()).collect();
        writer.write_record(&values).expect("write row");
    }
    writer.flush().expect("flush mismatch");

    Command::cargo_bin("csv-managed")
        .expect("binary exists")
        .args([
            "append",
            "-i",
            input.to_str().unwrap(),
            "-i",
            mismatch_path.to_str().unwrap(),
        ])
        .assert()
        .failure();
}

#[test]
fn verify_accepts_valid_big5_subset() {
    let temp = tempdir().expect("tempdir");
    let input = primary_dataset();
    let data = create_subset_with_checks(&temp, &input, &[(GOALS_COL, ColumnCheck::Integer)], 500);
    let schema_path =
        create_schema_with_overrides(&temp, &data, &[(GOALS_COL, ColumnType::Integer)]);

    Command::cargo_bin("csv-managed")
        .expect("binary exists")
        .args([
            "verify",
            "-m",
            schema_path.to_str().unwrap(),
            "-i",
            data.to_str().unwrap(),
        ])
        .assert()
        .success();
}

#[test]
fn verify_rejects_invalid_numeric_value() {
    let temp = tempdir().expect("tempdir");
    let input = primary_dataset();
    let data = create_subset_with_checks(&temp, &input, &[(GOALS_COL, ColumnCheck::Integer)], 500);
    let schema_path =
        create_schema_with_overrides(&temp, &data, &[(GOALS_COL, ColumnType::Integer)]);
    let broken = temp.path().join("broken.csv");

    let mut reader = ReaderBuilder::new()
        .has_headers(true)
        .from_path(&data)
        .expect("open fixture");
    let headers = reader.headers().expect("headers").clone();
    let goals_idx = headers
        .iter()
        .position(|h| h == GOALS_COL)
        .expect("goals index");
    let mut writer = WriterBuilder::new().from_path(&broken).expect("writer");
    writer.write_record(headers.iter()).expect("write headers");
    for (row_idx, record) in reader.records().enumerate().take(50) {
        let record = record.expect("record");
        let mut values: Vec<String> = record.iter().map(|v| v.to_string()).collect();
        if row_idx == 0 {
            values[goals_idx] = "not_a_number".to_string();
        }
        writer.write_record(&values).expect("write mutated row");
    }
    writer.flush().expect("flush broken");

    Command::cargo_bin("csv-managed")
        .expect("binary exists")
        .args([
            "verify",
            "-m",
            schema_path.to_str().unwrap(),
            "-i",
            broken.to_str().unwrap(),
        ])
        .assert()
        .failure()
        .stderr(
            contains("value \"not_a_number\"")
                .and(contains("Failed to parse 'not_a_number' as integer")),
        );
}

#[test]
fn verify_accepts_value_after_replacement() {
    let temp = tempdir().expect("tempdir");
    let input = primary_dataset();
    let data = create_subset_with_checks(&temp, &input, &[(GOALS_COL, ColumnCheck::Integer)], 500);
    let schema_path =
        create_schema_with_overrides(&temp, &data, &[(GOALS_COL, ColumnType::Integer)]);
    let broken = temp.path().join("broken.csv");

    let mut reader = ReaderBuilder::new()
        .has_headers(true)
        .from_path(&data)
        .expect("open fixture");
    let headers = reader.headers().expect("headers").clone();
    let goals_idx = headers
        .iter()
        .position(|h| h == GOALS_COL)
        .expect("goals index");
    let mut writer = WriterBuilder::new().from_path(&broken).expect("writer");
    writer.write_record(headers.iter()).expect("write headers");
    for (row_idx, record) in reader.records().enumerate().take(50) {
        let record = record.expect("record");
        let mut values: Vec<String> = record.iter().map(|v| v.to_string()).collect();
        if row_idx == 0 {
            values[goals_idx] = "not_a_number".to_string();
        }
        writer.write_record(&values).expect("write mutated row");
    }
    writer.flush().expect("flush broken");

    let mut schema_doc = Schema::load(&schema_path).expect("load schema");
    let column = schema_doc
        .columns
        .iter_mut()
        .find(|col| col.name == GOALS_COL)
        .expect("goals column");
    column.value_replacements.push(ValueReplacement {
        from: "not_a_number".to_string(),
        to: "0".to_string(),
    });
    schema_doc.save(&schema_path).expect("save schema");

    Command::cargo_bin("csv-managed")
        .expect("binary exists")
        .args([
            "verify",
            "-m",
            schema_path.to_str().unwrap(),
            "-i",
            broken.to_str().unwrap(),
        ])
        .assert()
        .success();
}

#[test]
fn stats_outputs_summary_for_selected_columns() {
    let temp = tempdir().expect("tempdir");
    let input = primary_dataset();
    let data = create_subset_with_checks(
        &temp,
        &input,
        &[
            (GOALS_COL, ColumnCheck::Integer),
            (ASSISTS_COL, ColumnCheck::Integer),
        ],
        500,
    );
    let schema_path = create_schema_with_overrides(
        &temp,
        &data,
        &[
            (GOALS_COL, ColumnType::Integer),
            (ASSISTS_COL, ColumnType::Integer),
        ],
    );

    let assert = Command::cargo_bin("csv-managed")
        .expect("binary exists")
        .args([
            "stats",
            "-i",
            data.to_str().unwrap(),
            "--schema",
            schema_path.to_str().unwrap(),
            "-C",
            GOALS_COL,
            "-C",
            ASSISTS_COL,
            "--limit",
            "200",
        ])
        .assert()
        .success();

    let output = String::from_utf8(assert.get_output().stdout.clone()).expect("stdout");
    assert!(output.contains(GOALS_COL));
    assert!(output.contains(ASSISTS_COL));
    assert!(output.contains("count"));
}

#[test]
fn frequency_outputs_top_values_for_boolean_column() {
    let temp = tempdir().expect("tempdir");
    let input = primary_dataset();
    let (data, schema_path) = create_boolean_subset(&temp, &input, 500);

    let assert = Command::cargo_bin("csv-managed")
        .expect("binary exists")
        .args([
            "frequency",
            "-i",
            data.to_str().unwrap(),
            "--schema",
            schema_path.to_str().unwrap(),
            "-C",
            BOOLEAN_COL,
            "--top",
            "3",
        ])
        .assert()
        .success();

    let output = String::from_utf8(assert.get_output().stdout.clone()).expect("stdout");
    assert!(output.contains(BOOLEAN_COL));
    assert!(output.contains("true"));
}

#[test]
fn process_boolean_format_true_false_outputs_normalized_values() {
    let temp = tempdir().expect("tempdir");
    let input = primary_dataset();
    let (data, schema_path) = create_boolean_subset(&temp, &input, 200);
    let output_path = temp.path().join("booleans_true_false.csv");

    Command::cargo_bin("csv-managed")
        .expect("binary exists")
        .args([
            "process",
            "-i",
            data.to_str().unwrap(),
            "-o",
            output_path.to_str().unwrap(),
            "--schema",
            schema_path.to_str().unwrap(),
            "--columns",
            BOOLEAN_COL,
            "--limit",
            "25",
            "--boolean-format",
            "true-false",
        ])
        .assert()
        .success();

    let (headers, rows) = read_csv(&output_path);
    assert_eq!(headers.iter().collect::<Vec<_>>(), vec![BOOLEAN_COL]);
    assert!(!rows.is_empty());
    for record in rows {
        let value = record.get(0).expect("boolean value");
        assert!(value == "true" || value == "false");
    }
}

#[test]
fn process_boolean_format_one_zero_outputs_digits() {
    let temp = tempdir().expect("tempdir");
    let input = primary_dataset();
    let (data, schema_path) = create_boolean_subset(&temp, &input, 200);
    let output_path = temp.path().join("booleans_one_zero.csv");

    Command::cargo_bin("csv-managed")
        .expect("binary exists")
        .args([
            "process",
            "-i",
            data.to_str().unwrap(),
            "-o",
            output_path.to_str().unwrap(),
            "--schema",
            schema_path.to_str().unwrap(),
            "--columns",
            BOOLEAN_COL,
            "--limit",
            "25",
            "--boolean-format",
            "one-zero",
        ])
        .assert()
        .success();

    let (headers, rows) = read_csv(&output_path);
    assert_eq!(headers.iter().collect::<Vec<_>>(), vec![BOOLEAN_COL]);
    assert!(!rows.is_empty());
    for record in rows {
        let value = record.get(0).expect("boolean value");
        assert!(value == "1" || value == "0");
    }
}

#[test]
fn preview_renders_requested_rows() {
    let input = primary_dataset();
    let assert = Command::cargo_bin("csv-managed")
        .expect("binary exists")
        .args(["preview", "-i", input.to_str().unwrap(), "--rows", "3"])
        .assert()
        .success();

    let output = String::from_utf8(assert.get_output().stdout.clone()).expect("stdout");
    assert!(output.contains(PLAYER_COL));
    assert!(output.contains(FIRST_PLAYER));
}
