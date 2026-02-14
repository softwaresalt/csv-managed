use std::{
    fs,
    fs::File,
    path::{Path, PathBuf},
};

use assert_cmd::Command;
use csv_managed::schema::{
    ColumnType, DecimalSpec, PlaceholderPolicy, Schema, infer_schema_with_stats,
};
use encoding_rs::UTF_8;
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
fn schema_infer_ignores_repeated_header_rows_in_big5_dataset() {
    let csv_path = fixture_path("big_5_players_stats_2023_2024.csv");
    let temp = tempdir().expect("temp dir");
    let schema_path = temp.path().join("big5_large_sample-schema.yml");

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
            "250",
        ])
        .assert()
        .success();

    let schema = Schema::load(&schema_path).expect("load inferred schema");
    let datatype_for = |name: &str| {
        schema
            .columns
            .iter()
            .find(|col| col.name == name)
            .unwrap_or_else(|| panic!("column {name} missing"))
            .datatype
            .clone()
    };

    assert_eq!(
        datatype_for("Rank"),
        ColumnType::Integer,
        "Rank column should remain integer when header rows repeat"
    );
    assert_eq!(
        datatype_for("Performance_Gls"),
        ColumnType::Integer,
        "Performance_Gls column should infer as integer"
    );
    let decimal_three_one = ColumnType::Decimal(DecimalSpec::new(3, 1).expect("decimal spec"));
    let decimal_three_two = ColumnType::Decimal(DecimalSpec::new(3, 2).expect("decimal spec"));
    assert_eq!(
        datatype_for("Expected_xG"),
        decimal_three_one,
        "Expected_xG column should infer as decimal(3,1)"
    );
    assert_eq!(
        datatype_for("Per 90 Minutes_xAG"),
        decimal_three_two,
        "Per 90 Minutes_xAG column should infer as decimal(3,2)"
    );
}

#[test]
fn schema_infer_does_not_record_headers_as_placeholder_tokens() {
    let csv_path = fixture_path("big_5_players_stats_2023_2024.csv");
    let policy = PlaceholderPolicy::default();

    let (_, stats) = infer_schema_with_stats(csv_path.as_path(), 250, b',', UTF_8, &policy, None)
        .expect("infer schema with stats");

    let rank_placeholders = stats
        .placeholder_summary(0)
        .map(|summary| summary.entries())
        .unwrap_or_default();
    assert!(
        rank_placeholders.is_empty(),
        "Rank column should not record header tokens as placeholders: {rank_placeholders:?}"
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
    let expected_score = ColumnType::Decimal(DecimalSpec::new(3, 1).expect("valid decimal spec"));
    assert_eq!(
        datatype_for("score"),
        expected_score,
        "score column should preserve decimal precision despite placeholder noise"
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
fn schema_infer_detects_headerless_dataset() {
    let csv_path = fixture_path("sensor_readings_no_header.csv");
    let policy = PlaceholderPolicy::default();
    let (schema, stats) =
        infer_schema_with_stats(csv_path.as_path(), 0, b',', UTF_8, &policy, None)
            .expect("infer schema for headerless input");

    assert!(
        !schema.expects_headers(),
        "schema should mark headerless input"
    );
    assert_eq!(schema.columns.len(), 3);
    assert_eq!(schema.columns[0].name, "field_0");
    assert_eq!(schema.columns[1].name, "field_1");
    assert_eq!(schema.columns[2].name, "field_2");
    assert_eq!(stats.rows_read(), 3);
}

#[test]
fn schema_infer_marks_headered_dataset() {
    let csv_path = fixture_path("big_5_players_stats_2023_2024.csv");
    let policy = PlaceholderPolicy::default();
    let (schema, _stats) =
        infer_schema_with_stats(csv_path.as_path(), 0, b',', UTF_8, &policy, None)
            .expect("infer schema for dataset with headers");

    assert!(
        schema.expects_headers(),
        "schema should retain header expectation"
    );
    assert!(
        schema
            .columns
            .iter()
            .any(|column| column.name.eq_ignore_ascii_case("player"))
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

#[test]
fn schema_infer_preview_emits_yaml_template_without_writing() {
    let temp = tempdir().expect("temp dir");
    let csv_path = temp.path().join("preview.csv");
    fs::write(&csv_path, "id,name\n1,Ada\n2,Grace\n").expect("write csv");

    let schema_path = temp.path().join("preview-schema.yml");
    let assert = Command::cargo_bin("csv-managed")
        .expect("binary present")
        .args([
            "schema",
            "infer",
            "-i",
            csv_path.to_str().unwrap(),
            "--preview",
            "--replace-template",
            "-o",
            schema_path.to_str().unwrap(),
        ])
        .assert()
        .success();

    let stdout = String::from_utf8(assert.get_output().stdout.clone()).expect("stdout utf8");
    assert!(
        stdout.contains("Schema YAML Preview (not written)"),
        "preview banner missing: {stdout}"
    );
    assert!(
        stdout.contains("name: id"),
        "expected id column missing from preview YAML: {stdout}"
    );
    assert!(
        stdout.contains("replace: []"),
        "template replace array missing from preview YAML: {stdout}"
    );
    assert!(
        !schema_path.exists(),
        "schema file should not be written when previewing"
    );
}

#[test]
fn schema_infer_preview_includes_placeholder_replacements() {
    let temp = tempdir().expect("temp dir");
    let csv_path = temp.path().join("placeholders.csv");
    fs::write(&csv_path, "code,value\n001,NA\n002,#N/A\n003,N/A\n").expect("write csv");

    let assert = Command::cargo_bin("csv-managed")
        .expect("binary present")
        .args([
            "schema",
            "infer",
            "-i",
            csv_path.to_str().unwrap(),
            "--preview",
            "--na-behavior",
            "fill",
            "--na-fill",
            "NULL",
        ])
        .assert()
        .success();

    let stdout = String::from_utf8(assert.get_output().stdout.clone()).expect("stdout utf8");
    let has_hash_na = stdout.contains("- from: \"#N/A\"") || stdout.contains("- from: '#N/A'");
    assert!(has_hash_na, "expected #N/A replacement missing: {stdout}");

    let has_na = stdout.contains("- from: NA")
        || stdout.contains("- from: \"NA\"")
        || stdout.contains("- from: 'NA'");
    assert!(has_na, "expected NA replacement missing: {stdout}");

    let has_fill = stdout.contains("to: NULL")
        || stdout.contains("to: \"NULL\"")
        || stdout.contains("to: 'NULL'");
    assert!(has_fill, "expected fill target missing: {stdout}");
}

#[test]
fn schema_probe_shows_placeholder_fill_with_custom_value() {
    let temp = tempdir().expect("temp dir");
    let csv_path = temp.path().join("placeholders.csv");
    fs::write(&csv_path, "code,value\n001,NA\n002,#N/A\n003,N/A\n").expect("write csv");

    let assert = Command::cargo_bin("csv-managed")
        .expect("binary present")
        .args([
            "schema",
            "probe",
            "-i",
            csv_path.to_str().unwrap(),
            "--na-behavior",
            "fill",
            "--na-fill",
            "MISSING",
        ])
        .assert()
        .success();

    let stdout = String::from_utf8(assert.get_output().stdout.clone()).expect("stdout utf8");
    assert!(
        stdout.contains("Placeholder Suggestions"),
        "placeholder section missing from probe output: {stdout}"
    );
    assert!(
        stdout.contains("MISSING"),
        "custom fill value 'MISSING' missing from probe output: {stdout}"
    );
}

#[test]
fn schema_infer_diff_reports_changes_and_no_changes() {
    let temp = tempdir().expect("temp dir");
    let csv_path = temp.path().join("diff.csv");
    fs::write(&csv_path, "id\n1\n2\n3\n").expect("write csv");

    let baseline_path = temp.path().join("existing-schema.yml");

    Command::cargo_bin("csv-managed")
        .expect("binary present")
        .args([
            "schema",
            "infer",
            "-i",
            csv_path.to_str().unwrap(),
            "-o",
            baseline_path.to_str().unwrap(),
            "--sample-rows",
            "0",
        ])
        .assert()
        .success();

    let mut baseline_contents = fs::read_to_string(&baseline_path).expect("read baseline schema");
    let original_line = baseline_contents
        .lines()
        .find(|line| line.contains("datatype:"))
        .expect("datatype line present")
        .to_string();
    let modified_line = original_line.replacen("Integer", "String", 1);
    baseline_contents = baseline_contents.replacen(&original_line, &modified_line, 1);
    fs::write(&baseline_path, &baseline_contents).expect("write modified baseline");

    let diff_assert = Command::cargo_bin("csv-managed")
        .expect("binary present")
        .args([
            "schema",
            "infer",
            "-i",
            csv_path.to_str().unwrap(),
            "--sample-rows",
            "0",
            "--diff",
            baseline_path.to_str().unwrap(),
        ])
        .assert()
        .success();

    let diff_stdout =
        String::from_utf8(diff_assert.get_output().stdout.clone()).expect("stdout utf8");
    assert!(
        diff_stdout.contains("Schema Diff vs"),
        "diff header missing: {diff_stdout}"
    );
    assert!(
        diff_stdout.contains(&format!("-{}\n", modified_line)),
        "expected removal line missing: {diff_stdout}"
    );
    assert!(
        diff_stdout.contains(&format!("+{}\n", original_line)),
        "expected addition line missing: {diff_stdout}"
    );

    Command::cargo_bin("csv-managed")
        .expect("binary present")
        .args([
            "schema",
            "infer",
            "-i",
            csv_path.to_str().unwrap(),
            "-o",
            baseline_path.to_str().unwrap(),
            "--sample-rows",
            "0",
        ])
        .assert()
        .success();

    let no_diff_assert = Command::cargo_bin("csv-managed")
        .expect("binary present")
        .args([
            "schema",
            "infer",
            "-i",
            csv_path.to_str().unwrap(),
            "--sample-rows",
            "0",
            "--diff",
            baseline_path.to_str().unwrap(),
        ])
        .assert()
        .success();

    let no_diff_stdout =
        String::from_utf8(no_diff_assert.get_output().stdout.clone()).expect("stdout utf8");
    assert!(
        no_diff_stdout.contains("no changes detected"),
        "expected no-change message missing: {no_diff_stdout}"
    );
}

#[test]
fn schema_verify_validates_multiple_files_independently() {
    let temp = tempdir().expect("temp dir");
    let schema_path = temp.path().join("multi-schema.yml");
    let valid_path = temp.path().join("valid.csv");
    let also_valid_path = temp.path().join("also_valid.csv");
    let invalid_path = temp.path().join("invalid.csv");

    fs::write(&valid_path, "id,name\n1,Alice\n2,Bob\n").expect("write valid csv");
    fs::write(&also_valid_path, "id,name\n3,Charlie\n4,Diana\n").expect("write also_valid csv");
    fs::write(&invalid_path, "id,name\nnotanumber,Eve\n").expect("write invalid csv");

    Command::cargo_bin("csv-managed")
        .expect("binary present")
        .args([
            "schema",
            "infer",
            "-i",
            valid_path.to_str().unwrap(),
            "-o",
            schema_path.to_str().unwrap(),
            "--sample-rows",
            "0",
        ])
        .assert()
        .success();

    // Both valid files should pass verification together.
    Command::cargo_bin("csv-managed")
        .expect("binary present")
        .args([
            "schema",
            "verify",
            "-m",
            schema_path.to_str().unwrap(),
            "-i",
            valid_path.to_str().unwrap(),
            "-i",
            also_valid_path.to_str().unwrap(),
        ])
        .assert()
        .success();

    // Including an invalid file should cause failure.
    Command::cargo_bin("csv-managed")
        .expect("binary present")
        .args([
            "schema",
            "verify",
            "-m",
            schema_path.to_str().unwrap(),
            "-i",
            valid_path.to_str().unwrap(),
            "-i",
            invalid_path.to_str().unwrap(),
        ])
        .assert()
        .failure()
        .stderr(contains("notanumber"));
}
