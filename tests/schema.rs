use std::{fs::File, path::Path};

use assert_cmd::Command;
use predicates::str::contains;
use serde_json::Value;
use tempfile::tempdir;

fn load_schema(path: &Path) -> Value {
    let file = File::open(path).expect("open schema output");
    serde_json::from_reader(file).expect("parse schema json")
}

fn column(value: &Value, index: usize) -> &Value {
    value
        .get("columns")
        .and_then(Value::as_array)
        .and_then(|cols| cols.get(index))
        .expect("column exists")
}

#[test]
fn schema_command_creates_schema_from_repeated_columns() {
    let temp = tempdir().expect("temp dir");
    let output = temp.path().join("basic.schema");

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
    let output = temp.path().join("comma.schema");

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
    assert_eq!(schema["columns"].as_array().map(Vec::len), Some(3));
    assert_eq!(column(&schema, 1)["name"].as_str(), Some("name"));
    assert_eq!(column(&schema, 2)["datatype"].as_str(), Some("DateTime"));
}

#[test]
fn schema_command_emits_renames_and_replacements() {
    let temp = tempdir().expect("temp dir");
    let output = temp.path().join("renamed.schema");

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
    let replacements = status_column["replace"].as_array().expect("replace array");
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
    let output = temp.path().join("duplicate.schema");

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
    let output = temp.path().join("duplicate_output.schema");

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
    let output = temp.path().join("bad_type.schema");

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
    let output = temp.path().join("bad_replace.schema");

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
