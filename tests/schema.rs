use std::{
    fs,
    fs::File,
    path::{Path, PathBuf},
};

use assert_cmd::Command;
use csv_managed::schema::{ColumnType, Schema};
use predicates::str::contains;
use serde_yaml::Value;
use tempfile::tempdir;

fn load_schema(path: &Path) -> Value {
    let file = File::open(path).expect("open schema output");
    serde_yaml::from_reader(file).expect("parse schema yaml")
}

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("data")
        .join(name)
}

fn column(value: &Value, index: usize) -> &Value {
    value
        .get("columns")
        .and_then(Value::as_sequence)
        .and_then(|cols| cols.get(index))
        .expect("column exists")
}

#[test]
fn schema_command_creates_schema_from_repeated_columns() {
    let temp = tempdir().expect("temp dir");
    let output = temp.path().join("basic-schema.yml");

    Command::cargo_bin("csv-managed")
        .expect("binary present")
        .args([
            "schema",
            "-o",
            output.to_str().unwrap(),
            "-c",
            "id:integer",
            "-c",
            "name:string",
            "-c",
            "amount:float",
        ])
        .assert()
        .success();

    let schema = load_schema(&output);
    let id = column(&schema, 0);
    assert_eq!(id.get("name").and_then(Value::as_str), Some("id"));
    assert_eq!(id.get("datatype").and_then(Value::as_str), Some("Integer"));
    assert!(id.get("name_mapping").is_none());

    let amount = column(&schema, 2);
    assert_eq!(
        amount.get("datatype").and_then(Value::as_str),
        Some("Float")
    );
}

#[test]
fn schema_command_supports_comma_delimited_columns() {
    let temp = tempdir().expect("temp dir");
    let output = temp.path().join("comma-schema.yml");

    Command::cargo_bin("csv-managed")
        .expect("binary present")
        .args([
            "schema",
            "-o",
            output.to_str().unwrap(),
            "-c",
            "id:integer,name:string",
            "-c",
            "ordered_at:datetime",
        ])
        .assert()
        .success();

    let schema = load_schema(&output);
    assert_eq!(schema["columns"].as_sequence().map(Vec::len), Some(3));
    assert_eq!(column(&schema, 1)["name"].as_str(), Some("name"));
    assert_eq!(column(&schema, 2)["datatype"].as_str(), Some("DateTime"));
}

#[test]
fn schema_command_emits_renames_and_replacements() {
    let temp = tempdir().expect("temp dir");
    let output = temp.path().join("renamed-schema.yml");

    Command::cargo_bin("csv-managed")
        .expect("binary present")
        .args([
            "schema",
            "-o",
            output.to_str().unwrap(),
            "-c",
            "status:string->order_status",
            "-c",
            "created_at:datetime",
            "--replace",
            "status=pending->ready",
            "--replace",
            "status=unknown->ready",
            "--replace",
            "status=complete->completed",
        ])
        .assert()
        .success();

    let schema = load_schema(&output);
    let status_column = column(&schema, 0);
    assert_eq!(status_column["name"].as_str(), Some("status"));
    assert_eq!(status_column["name_mapping"].as_str(), Some("order_status"));
    let replacements = status_column["replace"]
        .as_sequence()
        .expect("replace array");
    assert_eq!(replacements.len(), 3);
    assert!(
        replacements
            .iter()
            .any(|entry| entry["from"].as_str() == Some("pending")
                && entry["to"].as_str() == Some("ready"))
    );
    assert!(
        replacements
            .iter()
            .any(|entry| entry["from"].as_str() == Some("unknown")
                && entry["to"].as_str() == Some("ready"))
    );
    assert!(
        replacements
            .iter()
            .any(|entry| entry["from"].as_str() == Some("complete")
                && entry["to"].as_str() == Some("completed"))
    );
}

#[test]
fn schema_command_rejects_duplicate_column_names() {
    let temp = tempdir().expect("temp dir");
    let output = temp.path().join("duplicate-schema.yml");

    Command::cargo_bin("csv-managed")
        .expect("binary present")
        .args([
            "schema",
            "-o",
            output.to_str().unwrap(),
            "-c",
            "id:integer",
            "-c",
            "id:string",
        ])
        .assert()
        .failure()
        .stderr(contains("Duplicate column name"));
}

#[test]
fn schema_command_rejects_duplicate_output_names() {
    let temp = tempdir().expect("temp dir");
    let output = temp.path().join("duplicate_output-schema.yml");

    Command::cargo_bin("csv-managed")
        .expect("binary present")
        .args([
            "schema",
            "-o",
            output.to_str().unwrap(),
            "-c",
            "code:string->identifier",
            "-c",
            "status:string->identifier",
        ])
        .assert()
        .failure()
        .stderr(contains("Duplicate output column name"));
}

#[test]
fn schema_command_rejects_unknown_column_type() {
    let temp = tempdir().expect("temp dir");
    let output = temp.path().join("bad_type-schema.yml");

    Command::cargo_bin("csv-managed")
        .expect("binary present")
        .args(["schema", "-o", output.to_str().unwrap(), "-c", "id:number"])
        .assert()
        .failure()
        .stderr(contains("Unknown column type"));
}

#[test]
fn schema_command_validates_replacement_column_names() {
    let temp = tempdir().expect("temp dir");
    let output = temp.path().join("bad_replace-schema.yml");

    Command::cargo_bin("csv-managed")
        .expect("binary present")
        .args([
            "schema",
            "-o",
            output.to_str().unwrap(),
            "-c",
            "status:string",
            "--replace",
            "missing=pending->ready",
        ])
        .assert()
        .failure()
        .stderr(contains("unknown column"));
}

#[test]
fn schema_probe_on_big5_reports_samples_and_formats() {
    let csv_path = fixture_path("big_5_players_stats_2023_2024.csv");

    let assert = Command::cargo_bin("csv-managed")
        .expect("binary present")
        .args([
            "schema",
            "probe",
            "-i",
            csv_path.to_str().unwrap(),
            "--sample-rows",
            "5",
        ])
        .assert()
        .success();

    let stdout = String::from_utf8(assert.get_output().stdout.clone()).expect("stdout utf8");
    assert!(
        stdout.contains("sample"),
        "probe table missing sample column: {stdout}"
    );
    assert!(
        stdout.contains("format"),
        "probe table missing format column: {stdout}"
    );
    assert!(
        stdout.contains("Max Aarons"),
        "expected player sample missing: {stdout}"
    );
    assert!(
        stdout.contains("Whole number") || stdout.contains("Decimal point"),
        "format hint missing: {stdout}"
    );
    assert!(
        stdout.contains("Sampled 5 row(s)"),
        "sampling footer missing: {stdout}"
    );
}

#[test]
fn schema_infer_with_overrides_and_mapping_on_big5() {
    let csv_path = fixture_path("big_5_players_stats_2023_2024.csv");
    let temp = tempdir().expect("temp dir");
    let schema_path = temp.path().join("big5_override-schema.yml");

    Command::cargo_bin("csv-managed")
        .expect("binary present")
        .args([
            "schema",
            "infer",
            "--mapping",
            "--override",
            "Performance_Gls:integer",
            "--override",
            "Per 90 Minutes_Gls:string",
            "-i",
            csv_path.to_str().unwrap(),
            "-o",
            schema_path.to_str().unwrap(),
            "--sample-rows",
            "10",
        ])
        .assert()
        .success();

    let schema = Schema::load(&schema_path).expect("load inferred schema");
    let perf_gls = schema
        .columns
        .iter()
        .find(|col| col.name == "Performance_Gls")
        .expect("Performance_Gls column");
    assert_eq!(
        perf_gls.datatype,
        ColumnType::Integer,
        "override should coerce Performance_Gls to integer"
    );

    let per90_gls = schema
        .columns
        .iter()
        .find(|col| col.name == "Per 90 Minutes_Gls")
        .expect("Per 90 Minutes_Gls column");
    assert_eq!(
        per90_gls.datatype,
        ColumnType::String,
        "override should coerce Per 90 Minutes_Gls to string"
    );
    assert_eq!(
        per90_gls.rename.as_deref(),
        Some("per_90_minutes_gls"),
        "mapping should add snake_case rename"
    );
}

#[test]
fn schema_infer_prefers_majority_datatypes_from_fixture() {
    let csv_path = fixture_path("majority_datatypes.csv");
    let temp = tempdir().expect("temp dir");
    let schema_path = temp.path().join("majority-schema.yml");

    Command::cargo_bin("csv-managed")
        .expect("binary present")
        .args([
            "schema",
            "infer",
            "-i",
            csv_path.to_str().unwrap(),
            "-o",
            schema_path.to_str().unwrap(),
            "--sample-rows",
            "0",
        ])
        .assert()
        .success();

    let schema = Schema::load(&schema_path).expect("load inferred schema");
    let datatype_for = |name: &str| {
        schema
            .columns
            .iter()
            .find(|col| col.name == name)
            .unwrap_or_else(|| panic!("column {} missing", name))
            .datatype
            .clone()
    };

    assert_eq!(
        datatype_for("id"),
        ColumnType::Integer,
        "id column should stay integer despite stray text"
    );
    assert_eq!(
        datatype_for("flag"),
        ColumnType::Boolean,
        "flag column should resolve to boolean tokens"
    );
    assert_eq!(
        datatype_for("score"),
        ColumnType::Float,
        "score column should promote to float despite placeholder noise"
    );
    assert_eq!(
        datatype_for("price"),
        ColumnType::Currency,
        "price column should identify currency even with non-currency noise"
    );
    assert_eq!(
        datatype_for("created_on"),
        ColumnType::Date,
        "created_on column should infer date from majority of values"
    );
}

#[test]
fn schema_probe_snapshot_writes_and_validates_layout() {
    let csv_path = fixture_path("big_5_players_stats_2023_2024.csv");
    let temp = tempfile::tempdir().expect("temp dir");
    let snapshot_path = temp.path().join("probe.snap");

    Command::cargo_bin("csv-managed")
        .expect("binary present")
        .args([
            "schema",
            "probe",
            "-i",
            csv_path.to_str().unwrap(),
            "--sample-rows",
            "5",
            "--snapshot",
            snapshot_path.to_str().unwrap(),
        ])
        .assert()
        .success();

    let snapshot = fs::read_to_string(&snapshot_path).expect("snapshot written");
    assert!(
        snapshot.contains("Sampled"),
        "snapshot missing footer: {snapshot}"
    );

    // Second run should succeed when snapshot matches.
    Command::cargo_bin("csv-managed")
        .expect("binary present")
        .args([
            "schema",
            "probe",
            "-i",
            csv_path.to_str().unwrap(),
            "--sample-rows",
            "5",
            "--snapshot",
            snapshot_path.to_str().unwrap(),
        ])
        .assert()
        .success();

    // Deliberately corrupt snapshot to ensure mismatch is detected.
    fs::write(&snapshot_path, "corrupted snapshot").expect("overwrite snapshot");
    Command::cargo_bin("csv-managed")
        .expect("binary present")
        .args([
            "schema",
            "probe",
            "-i",
            csv_path.to_str().unwrap(),
            "--sample-rows",
            "5",
            "--snapshot",
            snapshot_path.to_str().unwrap(),
        ])
        .assert()
        .failure()
        .stderr(contains("Probe output does not match snapshot"));
}

#[test]
fn schema_infer_snapshot_writes_and_validates_layout() {
    let csv_path = fixture_path("big_5_players_stats_2023_2024.csv");
    let temp = tempfile::tempdir().expect("temp dir");
    let snapshot_path = temp.path().join("infer.snap");
    let schema_path = temp.path().join("infer-schema.yml");

    Command::cargo_bin("csv-managed")
        .expect("binary present")
        .args([
            "schema",
            "infer",
            "-i",
            csv_path.to_str().unwrap(),
            "-o",
            schema_path.to_str().unwrap(),
            "--sample-rows",
            "5",
            "--snapshot",
            snapshot_path.to_str().unwrap(),
        ])
        .assert()
        .success();

    let snapshot = fs::read_to_string(&snapshot_path).expect("snapshot written");
    assert!(
        snapshot.contains("Sampled"),
        "snapshot missing footer: {snapshot}"
    );

    let schema_contents = fs::read_to_string(&schema_path).expect("schema written");
    assert!(
        schema_contents.contains("columns"),
        "schema output missing columns"
    );

    // Second run should still succeed when snapshot matches the rendered output.
    Command::cargo_bin("csv-managed")
        .expect("binary present")
        .args([
            "schema",
            "infer",
            "-i",
            csv_path.to_str().unwrap(),
            "-o",
            schema_path.to_str().unwrap(),
            "--sample-rows",
            "5",
            "--snapshot",
            snapshot_path.to_str().unwrap(),
        ])
        .assert()
        .success();

    // Corrupt snapshot to ensure mismatch detection fires.
    fs::write(&snapshot_path, "corrupted snapshot").expect("overwrite snapshot");
    Command::cargo_bin("csv-managed")
        .expect("binary present")
        .args([
            "schema",
            "infer",
            "-i",
            csv_path.to_str().unwrap(),
            "-o",
            schema_path.to_str().unwrap(),
            "--sample-rows",
            "5",
            "--snapshot",
            snapshot_path.to_str().unwrap(),
        ])
        .assert()
        .failure()
        .stderr(contains("Probe output does not match snapshot"));
}

#[test]
fn schema_verify_accepts_currency_dataset() {
    let csv_path = fixture_path("currency_transactions.csv");
    let schema_path = fixture_path("currency_transactions-schema.yml");

    Command::cargo_bin("csv-managed")
        .expect("binary present")
        .args([
            "schema",
            "verify",
            "-m",
            schema_path.to_str().unwrap(),
            "-i",
            csv_path.to_str().unwrap(),
        ])
        .assert()
        .success();
}

#[test]
fn schema_verify_rejects_invalid_currency_precision() {
    let csv_path = fixture_path("currency_transactions_invalid.csv");
    let schema_path = fixture_path("currency_transactions-schema.yml");

    Command::cargo_bin("csv-managed")
        .expect("binary present")
        .args([
            "schema",
            "verify",
            "-m",
            schema_path.to_str().unwrap(),
            "-i",
            csv_path.to_str().unwrap(),
        ])
        .assert()
        .failure()
        .stderr(contains("Parsing '5.678' as currency"));
}
