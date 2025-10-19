use std::path::{Path, PathBuf};

use assert_cmd::Command;
use csv::{ReaderBuilder, StringRecord, WriterBuilder};
use csv_managed::data::parse_typed_value;
use csv_managed::schema::{ColumnType, Schema, ValueReplacement};
use predicates::{prelude::PredicateBooleanExt, str::contains};
use tempfile::tempdir;

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("data")
        .join(name)
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
    Command::cargo_bin("csv-managed")
        .expect("binary exists")
        .args([
            "probe",
            "-i",
            input_str,
            "-m",
            schema_str,
            "--delimiter",
            "tab",
            "--sample-rows",
            "0",
        ])
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
    Boolean,
}

impl ColumnCheck {
    fn as_column_type(self) -> ColumnType {
        match self {
            ColumnCheck::Integer => ColumnType::Integer,
            ColumnCheck::Boolean => ColumnType::Boolean,
        }
    }
}

fn create_subset_with_checks(
    dir: &tempfile::TempDir,
    input: &Path,
    checks: &[(&str, ColumnCheck)],
    limit: usize,
) -> PathBuf {
    let subset = dir.path().join("subset.tsv");
    let mut reader = ReaderBuilder::new()
        .delimiter(b'\t')
        .has_headers(true)
        .from_path(input)
        .expect("open source for subset");
    let headers = reader.headers().expect("headers").clone();
    let mut writer = WriterBuilder::new()
        .delimiter(b'\t')
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
    let mut reader = ReaderBuilder::new()
        .delimiter(b'\t')
        .has_headers(true)
        .from_path(path)
        .expect("open csv for counting");
    reader.records().count()
}

fn read_csv(path: &Path) -> (StringRecord, Vec<StringRecord>) {
    let mut reader = ReaderBuilder::new()
        .delimiter(b'\t')
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

#[test]
fn probe_infers_expected_types_for_ipqs() {
    let temp = tempdir().expect("tempdir");
    let input = fixture_path("ipqs_nonfraud_signaldata.tsv");
    let schema_path = create_schema(&temp, &input);

    let schema = Schema::load(&schema_path).expect("load schema");
    let find_type = |name: &str| -> ColumnType {
        schema
            .columns
            .iter()
            .find(|column| column.name == name)
            .map(|column| column.datatype.clone())
            .expect("column present")
    };

    assert_eq!(find_type("emailName"), ColumnType::String);
    assert!(schema.columns.len() > 80);
    let boolean_columns = schema
        .columns
        .iter()
        .filter(|column| matches!(column.datatype, ColumnType::Boolean))
        .count();
    assert!(boolean_columns > 0);
    let numeric_columns = schema
        .columns
        .iter()
        .filter(|column| matches!(column.datatype, ColumnType::Integer | ColumnType::Float))
        .count();
    assert!(numeric_columns > 0);
}

#[test]
fn process_with_index_respects_sort_order() {
    let temp = tempdir().expect("tempdir");
    let input = fixture_path("ipqs_nonfraud_signaldata.tsv");
    let data = create_subset_with_checks(
        &temp,
        &input,
        &[
            ("ipqs_email_Fraud Score", ColumnCheck::Integer),
            ("ipqs_phone_Fraud Score", ColumnCheck::Integer),
        ],
        5000,
    );
    let schema_path = create_schema_with_overrides(
        &temp,
        &data,
        &[
            ("ipqs_email_Fraud Score", ColumnType::Integer),
            ("ipqs_phone_Fraud Score", ColumnType::Integer),
        ],
    );
    let index_path = temp.path().join("data.idx");
    let output_path = temp.path().join("sorted.tsv");

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
            "--delimiter",
            "tab",
            "--spec",
            "ipqs_email_Fraud Score:desc,ipqs_phone_Fraud Score:asc",
        ])
        .assert()
        .success();

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
            "--delimiter",
            "tab",
            "--sort",
            "ipqs_email_Fraud Score:desc",
            "--sort",
            "ipqs_phone_Fraud Score:asc",
            "--columns",
            "GUID",
            "--columns",
            "ipqs_email_Fraud Score",
            "--columns",
            "ipqs_phone_Fraud Score",
            "--row-numbers",
            "--limit",
            "25",
            "--output-delimiter",
            "tab",
        ])
        .assert()
        .success();

    let (headers, rows) = read_csv(&output_path);
    let row_number_idx = headers
        .iter()
        .position(|h| h == "row_number")
        .expect("row number header");
    let email_idx = headers
        .iter()
        .position(|h| h == "ipqs_email_Fraud Score")
        .expect("email fraud score header");

    let mut email_scores = Vec::new();
    for (idx, record) in rows.iter().enumerate() {
        let row_number: usize = record
            .get(row_number_idx)
            .expect("row number value")
            .parse()
            .expect("row number parse");
        assert_eq!(row_number, idx + 1);
        let email_score: i64 = record
            .get(email_idx)
            .expect("email score value")
            .parse()
            .expect("email score parse");
        email_scores.push(email_score);
    }
    assert!(email_scores.windows(2).all(|pair| pair[0] >= pair[1]));
}

#[test]
fn process_filters_and_derives_high_risk_segment() {
    let temp = tempdir().expect("tempdir");
    let input = fixture_path("ipqs_nonfraud_signaldata.tsv");
    let data = create_subset_with_checks(
        &temp,
        &input,
        &[("ipqs_email_Fraud Score", ColumnCheck::Integer)],
        5000,
    );
    let schema_path = create_schema_with_overrides(
        &temp,
        &data,
        &[
            ("ipqs_email_Fraud Score", ColumnType::Integer),
            ("ipqs_phone_Fraud Score", ColumnType::Integer),
        ],
    );
    let output_path = temp.path().join("filtered.tsv");

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
            "--delimiter",
            "tab",
            "--filter",
            "ipqs_email_Fraud Score >= 90",
            "--derive",
            "risk_flag=ipqs_email_fraud_score >= 90",
            "--columns",
            "GUID",
            "--columns",
            "ipqs_email_Fraud Score",
            "--limit",
            "10",
            "--output-delimiter",
            "tab",
        ])
        .assert()
        .success();

    let (headers, rows) = read_csv(&output_path);
    let email_idx = headers
        .iter()
        .position(|h| h == "ipqs_email_Fraud Score")
        .expect("email fraud header");
    let risk_idx = headers
        .iter()
        .position(|h| h == "risk_flag")
        .expect("risk flag header");

    assert!(!rows.is_empty());
    for record in rows {
        let score: i64 = record
            .get(email_idx)
            .expect("score value")
            .parse()
            .expect("score parse");
        assert!(score >= 90);
        assert_eq!(record.get(risk_idx).expect("risk"), "true");
    }
}

#[test]
fn append_merges_ipqs_datasets() {
    let temp = tempdir().expect("tempdir");
    let input_a = fixture_path("ipqs_nonfraud_signaldata.tsv");
    let output_path = temp.path().join("appended.tsv");

    let subset_path = temp.path().join("subset.tsv");
    {
        let mut reader = ReaderBuilder::new()
            .delimiter(b'\t')
            .has_headers(true)
            .from_path(&input_a)
            .expect("open source for subset");
        let headers = reader.headers().expect("headers").clone();
        let mut writer = WriterBuilder::new()
            .delimiter(b'\t')
            .from_path(&subset_path)
            .expect("subset writer");
        writer.write_record(headers.iter()).expect("subset headers");
        for record in reader.records().take(500) {
            let record = record.expect("record");
            let values: Vec<String> = record.iter().map(|v| v.to_string()).collect();
            writer.write_record(&values).expect("subset row");
        }
        writer.flush().expect("flush subset");
    }

    let expected_rows = count_rows(&input_a) + count_rows(&subset_path);

    Command::cargo_bin("csv-managed")
        .expect("binary exists")
        .args([
            "append",
            "-i",
            input_a.to_str().unwrap(),
            "-i",
            subset_path.to_str().unwrap(),
            "-o",
            output_path.to_str().unwrap(),
            "--delimiter",
            "tab",
        ])
        .assert()
        .success();

    let actual_rows = count_rows(&output_path);
    assert_eq!(actual_rows, expected_rows);
}

#[test]
fn append_rejects_mismatched_headers() {
    let input = fixture_path("ipqs_nonfraud_signaldata.tsv");
    let second = fixture_path("ipqs_fraud_signaldata.tsv");

    Command::cargo_bin("csv-managed")
        .expect("binary exists")
        .args([
            "append",
            "-i",
            input.to_str().unwrap(),
            "-i",
            second.to_str().unwrap(),
            "--delimiter",
            "tab",
        ])
        .assert()
        .failure();
}

#[test]
fn verify_accepts_valid_ipqs_files() {
    let temp = tempdir().expect("tempdir");
    let input_a = fixture_path("ipqs_nonfraud_signaldata.tsv");
    let data = create_subset_with_checks(
        &temp,
        &input_a,
        &[("ipqs_email_Fraud Score", ColumnCheck::Integer)],
        5000,
    );
    let schema_path = create_schema_with_overrides(
        &temp,
        &data,
        &[("ipqs_email_Fraud Score", ColumnType::Integer)],
    );

    Command::cargo_bin("csv-managed")
        .expect("binary exists")
        .args([
            "verify",
            "-m",
            schema_path.to_str().unwrap(),
            "-i",
            data.to_str().unwrap(),
            "--delimiter",
            "tab",
        ])
        .assert()
        .success();
}

#[test]
fn verify_rejects_invalid_numeric_value() {
    let temp = tempdir().expect("tempdir");
    let input = fixture_path("ipqs_nonfraud_signaldata.tsv");
    let data = create_subset_with_checks(
        &temp,
        &input,
        &[("ipqs_email_Fraud Score", ColumnCheck::Integer)],
        5000,
    );
    let schema_path = create_schema_with_overrides(
        &temp,
        &data,
        &[("ipqs_email_Fraud Score", ColumnType::Integer)],
    );
    let broken = temp.path().join("broken.tsv");

    let mut reader = ReaderBuilder::new()
        .delimiter(b'\t')
        .has_headers(true)
        .from_path(&data)
        .expect("open fixture");
    let headers = reader.headers().expect("headers").clone();
    let fraud_idx = headers
        .iter()
        .position(|h| h == "ipqs_email_Fraud Score")
        .expect("fraud score index");
    let mut writer = WriterBuilder::new()
        .delimiter(b'\t')
        .from_path(&broken)
        .expect("writer");
    writer.write_record(headers.iter()).expect("write headers");
    for (row_idx, record) in reader.records().enumerate().take(50) {
        let record = record.expect("record");
        let mut values: Vec<String> = record.iter().map(|v| v.to_string()).collect();
        if row_idx == 0 {
            values[fraud_idx] = "not_a_number".to_string();
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
            "--delimiter",
            "tab",
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
    let input = fixture_path("ipqs_nonfraud_signaldata.tsv");
    let data = create_subset_with_checks(
        &temp,
        &input,
        &[("ipqs_email_Fraud Score", ColumnCheck::Integer)],
        5000,
    );
    let schema_path = create_schema_with_overrides(
        &temp,
        &data,
        &[("ipqs_email_Fraud Score", ColumnType::Integer)],
    );
    let broken = temp.path().join("broken.tsv");

    let mut reader = ReaderBuilder::new()
        .delimiter(b'\t')
        .has_headers(true)
        .from_path(&data)
        .expect("open fixture");
    let headers = reader.headers().expect("headers").clone();
    let fraud_idx = headers
        .iter()
        .position(|h| h == "ipqs_email_Fraud Score")
        .expect("fraud score index");
    let mut writer = WriterBuilder::new()
        .delimiter(b'\t')
        .from_path(&broken)
        .expect("writer");
    writer.write_record(headers.iter()).expect("write headers");
    for (row_idx, record) in reader.records().enumerate().take(50) {
        let record = record.expect("record");
        let mut values: Vec<String> = record.iter().map(|v| v.to_string()).collect();
        if row_idx == 0 {
            values[fraud_idx] = "not_a_number".to_string();
        }
        writer.write_record(&values).expect("write mutated row");
    }
    writer.flush().expect("flush broken");

    let mut schema_doc = Schema::load(&schema_path).expect("load schema");
    let column = schema_doc
        .columns
        .iter_mut()
        .find(|col| col.name == "ipqs_email_Fraud Score")
        .expect("fraud column");
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
            "--delimiter",
            "tab",
        ])
        .assert()
        .success();
}

#[test]
fn stats_outputs_summary_for_selected_columns() {
    let temp = tempdir().expect("tempdir");
    let input = fixture_path("ipqs_nonfraud_signaldata.tsv");
    let data = create_subset_with_checks(
        &temp,
        &input,
        &[
            ("ipqs_email_Fraud Score", ColumnCheck::Integer),
            ("ipqs_phone_Fraud Score", ColumnCheck::Integer),
        ],
        5000,
    );
    let schema_path = create_schema_with_overrides(
        &temp,
        &data,
        &[
            ("ipqs_email_Fraud Score", ColumnType::Integer),
            ("ipqs_phone_Fraud Score", ColumnType::Integer),
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
            "--delimiter",
            "tab",
            "-C",
            "ipqs_email_Fraud Score",
            "-C",
            "ipqs_phone_Fraud Score",
            "--limit",
            "200",
        ])
        .assert()
        .success();

    let output = String::from_utf8(assert.get_output().stdout.clone()).expect("stdout");
    assert!(output.contains("ipqs_email_Fraud Score"));
    assert!(output.contains("ipqs_phone_Fraud Score"));
    assert!(output.contains("count"));
}

#[test]
fn frequency_outputs_top_values_for_boolean_column() {
    let temp = tempdir().expect("tempdir");
    let input = fixture_path("ipqs_nonfraud_signaldata.tsv");
    let data = create_subset_with_checks(
        &temp,
        &input,
        &[("ipqs_email_Valid", ColumnCheck::Boolean)],
        5000,
    );
    let schema_path =
        create_schema_with_overrides(&temp, &data, &[("ipqs_email_Valid", ColumnType::Boolean)]);

    let assert = Command::cargo_bin("csv-managed")
        .expect("binary exists")
        .args([
            "frequency",
            "-i",
            data.to_str().unwrap(),
            "--schema",
            schema_path.to_str().unwrap(),
            "--delimiter",
            "tab",
            "-C",
            "ipqs_email_Valid",
            "--top",
            "3",
        ])
        .assert()
        .success();

    let output = String::from_utf8(assert.get_output().stdout.clone()).expect("stdout");
    assert!(output.contains("ipqs_email_Valid"));
    assert!(output.contains("true"));
}

#[test]
fn process_boolean_format_true_false_outputs_normalized_values() {
    let temp = tempdir().expect("tempdir");
    let input = fixture_path("ipqs_nonfraud_signaldata.tsv");
    let data = create_subset_with_checks(
        &temp,
        &input,
        &[("ipqs_email_Valid", ColumnCheck::Boolean)],
        2000,
    );
    let schema_path =
        create_schema_with_overrides(&temp, &data, &[("ipqs_email_Valid", ColumnType::Boolean)]);
    let output_path = temp.path().join("booleans_true_false.tsv");

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
            "--delimiter",
            "tab",
            "--columns",
            "ipqs_email_Valid",
            "--limit",
            "25",
            "--boolean-format",
            "true-false",
            "--output-delimiter",
            "tab",
        ])
        .assert()
        .success();

    let (headers, rows) = read_csv(&output_path);
    assert_eq!(headers.iter().collect::<Vec<_>>(), vec!["ipqs_email_Valid"]);
    assert!(!rows.is_empty());
    for record in rows {
        let value = record.get(0).expect("boolean value");
        assert!(value == "true" || value == "false");
    }
}

#[test]
fn process_boolean_format_one_zero_outputs_digits() {
    let temp = tempdir().expect("tempdir");
    let input = fixture_path("ipqs_nonfraud_signaldata.tsv");
    let data = create_subset_with_checks(
        &temp,
        &input,
        &[("ipqs_email_Valid", ColumnCheck::Boolean)],
        2000,
    );
    let schema_path =
        create_schema_with_overrides(&temp, &data, &[("ipqs_email_Valid", ColumnType::Boolean)]);
    let output_path = temp.path().join("booleans_one_zero.tsv");

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
            "--delimiter",
            "tab",
            "--columns",
            "ipqs_email_Valid",
            "--limit",
            "25",
            "--boolean-format",
            "one-zero",
            "--output-delimiter",
            "tab",
        ])
        .assert()
        .success();

    let (headers, rows) = read_csv(&output_path);
    assert_eq!(headers.iter().collect::<Vec<_>>(), vec!["ipqs_email_Valid"]);
    assert!(!rows.is_empty());
    for record in rows {
        let value = record.get(0).expect("boolean value");
        assert!(value == "1" || value == "0");
    }
}

#[test]
fn preview_renders_requested_rows() {
    let input = fixture_path("ipqs_nonfraud_signaldata.tsv");
    let assert = Command::cargo_bin("csv-managed")
        .expect("binary exists")
        .args([
            "preview",
            "-i",
            input.to_str().unwrap(),
            "--delimiter",
            "tab",
            "--rows",
            "3",
        ])
        .assert()
        .success();

    let output = String::from_utf8(assert.get_output().stdout.clone()).expect("stdout");
    assert!(output.contains("GUID"));
    assert!(output.contains("dyanash"));
}
