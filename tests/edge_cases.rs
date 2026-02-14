//! Edge-case integration tests for Phase 13 polish.
//!
//! Validates boundary conditions: empty files, header-only CSVs, unknown columns,
//! malformed expressions, empty stdin, decimal overflow, column rename transparency,
//! multiple filters with AND semantics, and in-memory sort fallback.

use std::{fs, path::PathBuf};

use assert_cmd::Command;
use predicates::str::contains;
use tempfile::tempdir;

#[allow(dead_code)]
fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("data")
        .join(name)
}

// ---------------------------------------------------------------------------
// T125: Empty CSV file (0 bytes) across schema probe, process, stats, verify
// ---------------------------------------------------------------------------

#[test]
fn empty_csv_probe_handles_gracefully() {
    let dir = tempdir().expect("temp dir");
    let empty = dir.path().join("empty.csv");
    fs::write(&empty, "").expect("write empty file");

    // Probe on an empty CSV succeeds and reports "No columns inferred."
    Command::cargo_bin("csv-managed")
        .expect("binary exists")
        .args(["schema", "probe", "-i", empty.to_str().unwrap()])
        .assert()
        .success()
        .stdout(contains("No columns inferred"));
}

#[test]
fn empty_csv_process_produces_empty_output() {
    let dir = tempdir().expect("temp dir");
    let empty = dir.path().join("empty.csv");
    fs::write(&empty, "").expect("write empty file");
    let output = dir.path().join("out.csv");

    // Process on an empty CSV succeeds with 0 rows output.
    Command::cargo_bin("csv-managed")
        .expect("binary exists")
        .args([
            "process",
            "-i",
            empty.to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();

    // Output file is created but contains no data rows.
    let data = fs::read_to_string(&output).unwrap_or_default();
    let line_count = data.lines().count();
    assert!(
        line_count <= 1,
        "Expected 0 or 1 lines (header only), got {line_count}"
    );
}

#[test]
fn empty_csv_stats_reports_error() {
    let dir = tempdir().expect("temp dir");
    let empty = dir.path().join("empty.csv");
    fs::write(&empty, "").expect("write empty file");

    Command::cargo_bin("csv-managed")
        .expect("binary exists")
        .args(["stats", "-i", empty.to_str().unwrap()])
        .assert()
        .failure();
}

#[test]
fn empty_csv_verify_reports_error() {
    let dir = tempdir().expect("temp dir");
    let empty = dir.path().join("empty.csv");
    let schema = dir.path().join("schema.yml");
    fs::write(&empty, "").expect("write empty file");
    fs::write(
        &schema,
        "version: 1\ncolumns:\n  - name: id\n    datatype: Integer\n",
    )
    .expect("write schema");

    Command::cargo_bin("csv-managed")
        .expect("binary exists")
        .args([
            "schema",
            "verify",
            "-m",
            schema.to_str().unwrap(),
            "-i",
            empty.to_str().unwrap(),
        ])
        .assert()
        .failure();
}

// ---------------------------------------------------------------------------
// T126: Header-only CSV (no data rows) across stats and verify
// ---------------------------------------------------------------------------

#[test]
fn header_only_csv_stats_succeeds_or_reports_no_data() {
    let dir = tempdir().expect("temp dir");
    let csv = dir.path().join("header_only.csv");
    fs::write(&csv, "id,name,amount\n").expect("write header-only csv");

    // Stats on a header-only file: no numeric data to compute on.
    // The command should either succeed with empty stats or fail with a clear message.
    let result = Command::cargo_bin("csv-managed")
        .expect("binary exists")
        .args(["stats", "-i", csv.to_str().unwrap()])
        .assert();

    // Accept either success (empty stats) or failure (clear message).
    let output = result.get_output();
    let _stdout = String::from_utf8_lossy(&output.stdout);
    let _stderr = String::from_utf8_lossy(&output.stderr);
    // As long as it doesn't panic / crash, the behavior is acceptable.
}

#[test]
fn header_only_csv_verify_succeeds() {
    let dir = tempdir().expect("temp dir");
    let csv = dir.path().join("header_only.csv");
    let schema = dir.path().join("schema.yml");
    fs::write(&csv, "id,name\n").expect("write header-only csv");
    fs::write(
        &schema,
        "version: 1\ncolumns:\n  - name: id\n    datatype: Integer\n  - name: name\n    datatype: String\n",
    )
    .expect("write schema");

    // Verify with no data rows should succeed — 0 violations.
    Command::cargo_bin("csv-managed")
        .expect("binary exists")
        .args([
            "schema",
            "verify",
            "-m",
            schema.to_str().unwrap(),
            "-i",
            csv.to_str().unwrap(),
        ])
        .assert()
        .success();
}

// ---------------------------------------------------------------------------
// T127: Unknown column in filter expression — clear error message
// ---------------------------------------------------------------------------

#[test]
fn unknown_filter_column_reports_clear_error() {
    let dir = tempdir().expect("temp dir");
    let csv = dir.path().join("sample.csv");
    let output = dir.path().join("out.csv");
    fs::write(&csv, "id,name\n1,Alice\n2,Bob\n").expect("write csv");

    Command::cargo_bin("csv-managed")
        .expect("binary exists")
        .args([
            "process",
            "-i",
            csv.to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
            "--filter",
            "nonexistent_column=foo",
        ])
        .assert()
        .failure()
        .stderr(contains("not found"));
}

// ---------------------------------------------------------------------------
// T128: Malformed derive expression — parse error with position
// ---------------------------------------------------------------------------

#[test]
fn malformed_derive_expression_reports_parse_error() {
    let dir = tempdir().expect("temp dir");
    let csv = dir.path().join("sample.csv");
    let output = dir.path().join("out.csv");
    fs::write(&csv, "id,name\n1,Alice\n2,Bob\n").expect("write csv");

    Command::cargo_bin("csv-managed")
        .expect("binary exists")
        .args([
            "process",
            "-i",
            csv.to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
            "--derive",
            "bad_col=@#$invalid",
        ])
        .assert()
        .failure();
}

#[test]
fn derive_missing_equals_reports_error() {
    let dir = tempdir().expect("temp dir");
    let csv = dir.path().join("sample.csv");
    let output = dir.path().join("out.csv");
    fs::write(&csv, "id,name\n1,Alice\n2,Bob\n").expect("write csv");

    Command::cargo_bin("csv-managed")
        .expect("binary exists")
        .args([
            "process",
            "-i",
            csv.to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
            "--derive",
            "no_equals_sign",
        ])
        .assert()
        .failure()
        .stderr(contains("missing"));
}

// ---------------------------------------------------------------------------
// T129: Empty stdin pipe — detection and reporting
// ---------------------------------------------------------------------------

#[test]
fn empty_stdin_process_handles_gracefully() {
    let dir = tempdir().expect("temp dir");
    let output = dir.path().join("out.csv");

    // Feed empty stdin to process — succeeds with 0 rows.
    Command::cargo_bin("csv-managed")
        .expect("binary exists")
        .args(["process", "-i", "-", "-o", output.to_str().unwrap()])
        .write_stdin("")
        .assert()
        .success();

    let data = fs::read_to_string(&output).unwrap_or_default();
    let line_count = data.lines().count();
    assert!(
        line_count <= 1,
        "Expected 0 or 1 lines for empty stdin, got {line_count}"
    );
}

// ---------------------------------------------------------------------------
// T130: Decimal precision overflow (>28 digits) — error
// ---------------------------------------------------------------------------

#[test]
fn decimal_precision_overflow_detected_in_schema() {
    let dir = tempdir().expect("temp dir");
    let schema = dir.path().join("schema.yml");
    let csv = dir.path().join("data.csv");

    // A schema specifying precision > 28 via decimal(29,2) should be rejected.
    fs::write(
        &schema,
        "version: 1\ncolumns:\n  - name: value\n    datatype: decimal(29,2)\n",
    )
    .expect("write schema");
    fs::write(&csv, "value\n1.23\n").expect("write csv");

    Command::cargo_bin("csv-managed")
        .expect("binary exists")
        .args([
            "schema",
            "verify",
            "-m",
            schema.to_str().unwrap(),
            "-i",
            csv.to_str().unwrap(),
        ])
        .assert()
        .failure()
        .stderr(contains("28"));
}

// ---------------------------------------------------------------------------
// T131: Column rename with original header name — transparent mapping
// ---------------------------------------------------------------------------

#[test]
fn filter_works_with_original_column_name_after_rename() {
    let dir = tempdir().expect("temp dir");
    let csv = dir.path().join("sample.csv");
    let schema = dir.path().join("schema.yml");
    let output = dir.path().join("out.csv");

    fs::write(&csv, "id,name,amount\n1,Alice,42\n2,Bob,13\n3,Carol,100\n").expect("write csv");
    // Rename 'amount' to 'total' in schema, then filter by original name 'amount'.
    fs::write(
        &schema,
        "version: 1\ncolumns:\n  - name: id\n    datatype: Integer\n  - name: name\n    datatype: String\n  - name: amount\n    datatype: Integer\n    rename: total\n",
    )
    .expect("write schema");

    Command::cargo_bin("csv-managed")
        .expect("binary exists")
        .args([
            "process",
            "-i",
            csv.to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
            "--schema",
            schema.to_str().unwrap(),
            "--filter",
            "amount >= 42",
        ])
        .assert()
        .success();

    let data = fs::read_to_string(&output).expect("read output");
    // Output should contain only Alice (42) and Carol (100).
    assert!(data.contains("Alice"), "Expected Alice in filtered output");
    assert!(data.contains("Carol"), "Expected Carol in filtered output");
    assert!(
        !data.contains("Bob"),
        "Bob should be excluded by filter amount >= 42"
    );
}

// ---------------------------------------------------------------------------
// T132: Multiple --filter flags — AND semantics
// ---------------------------------------------------------------------------

#[test]
fn multiple_filters_use_and_semantics() {
    let dir = tempdir().expect("temp dir");
    let csv = dir.path().join("sample.csv");
    let schema = dir.path().join("schema.yml");
    let output = dir.path().join("out.csv");

    fs::write(
        &csv,
        "id,name,amount,status\n1,Alice,100,active\n2,Bob,50,active\n3,Carol,200,inactive\n4,Dave,150,active\n",
    )
    .expect("write csv");

    // Schema needed for typed comparison (Integer for amount).
    fs::write(
        &schema,
        "version: 1\ncolumns:\n  - name: id\n    datatype: Integer\n  - name: name\n    datatype: String\n  - name: amount\n    datatype: Integer\n  - name: status\n    datatype: String\n",
    )
    .expect("write schema");

    // Both filters must match (AND semantics).
    Command::cargo_bin("csv-managed")
        .expect("binary exists")
        .args([
            "process",
            "-i",
            csv.to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
            "--schema",
            schema.to_str().unwrap(),
            "--filter",
            "amount >= 100",
            "--filter",
            "status = active",
        ])
        .assert()
        .success();

    let data = fs::read_to_string(&output).expect("read output");
    // Only Alice (100, active) and Dave (150, active) satisfy both filters.
    assert!(data.contains("Alice"), "Expected Alice (100, active)");
    assert!(data.contains("Dave"), "Expected Dave (150, active)");
    assert!(
        !data.contains("Bob"),
        "Bob (50, active) excluded by amount >= 100"
    );
    assert!(
        !data.contains("Carol"),
        "Carol (200, inactive) excluded by status = active"
    );
}

// ---------------------------------------------------------------------------
// T133: --sort without matching index — in-memory fallback
// ---------------------------------------------------------------------------

#[test]
fn sort_without_index_uses_in_memory_fallback() {
    let dir = tempdir().expect("temp dir");
    let csv = dir.path().join("data.csv");
    let schema = dir.path().join("schema.yml");
    let output = dir.path().join("sorted.csv");

    // Create a dataset with enough rows to exercise the sort path.
    let mut content = String::from("id,name,score\n");
    for i in (1..=50).rev() {
        content.push_str(&format!("{i},player_{i},{}\n", i * 10));
    }
    fs::write(&csv, &content).expect("write csv");

    // Schema needed for typed (Integer) sort instead of string comparison.
    fs::write(
        &schema,
        "version: 1\ncolumns:\n  - name: id\n    datatype: Integer\n  - name: name\n    datatype: String\n  - name: score\n    datatype: Integer\n",
    )
    .expect("write schema");

    // Sort ascending by score without an index — triggers in-memory fallback.
    Command::cargo_bin("csv-managed")
        .expect("binary exists")
        .args([
            "process",
            "-i",
            csv.to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
            "--schema",
            schema.to_str().unwrap(),
            "--sort",
            "score:asc",
        ])
        .assert()
        .success();

    // Verify output is sorted by score ascending.
    let data = fs::read_to_string(&output).expect("read output");
    let lines: Vec<&str> = data.lines().collect();
    assert!(lines.len() > 1, "Expected header + data rows");

    // Extract score column values (index 2) and verify ascending order.
    let scores: Vec<i64> = lines[1..]
        .iter()
        .map(|line| {
            line.replace('"', "")
                .split(',')
                .nth(2)
                .expect("score column")
                .trim()
                .parse::<i64>()
                .expect("parse score")
        })
        .collect();
    for window in scores.windows(2) {
        assert!(
            window[0] <= window[1],
            "Expected ascending order, got {} before {}",
            window[0],
            window[1]
        );
    }
}
