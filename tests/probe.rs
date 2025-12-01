use std::{fs, path::PathBuf};

use assert_cmd::cargo::cargo_bin_cmd;
use csv_managed::schema::{ColumnType, Schema};
use encoding_rs::WINDOWS_1252;
use tempfile::tempdir;

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("data")
        .join(name)
}

#[test]
fn probe_full_scan_detects_string_in_mixed_column() {
    let temp = tempdir().expect("temp dir");
    let schema_path = temp.path().join("mixed_full-schema.yml");
    cargo_bin_cmd!("csv-managed")
        .args([
            "schema",
            "infer",
            "-i",
            fixture_path("probe_sample_variation.csv").to_str().unwrap(),
            "-o",
            schema_path.to_str().unwrap(),
            "--sample-rows",
            "0",
        ])
        .assert()
        .success();

    let schema = Schema::load(&schema_path).expect("load inferred schema");
    let mixed_column = schema
        .columns
        .iter()
        .find(|col| col.name == "mixed")
        .expect("mixed column present");
    assert_eq!(mixed_column.datatype, ColumnType::String);
}

#[test]
fn probe_sampling_can_limit_type_detection() {
    let temp = tempdir().expect("temp dir");
    let schema_path = temp.path().join("mixed_sampled-schema.yml");
    cargo_bin_cmd!("csv-managed")
        .args([
            "schema",
            "infer",
            "-i",
            fixture_path("probe_sample_variation.csv").to_str().unwrap(),
            "-o",
            schema_path.to_str().unwrap(),
            "--sample-rows",
            "1",
        ])
        .assert()
        .success();

    let schema = Schema::load(&schema_path).expect("load sampled schema");
    let mixed_column = schema
        .columns
        .iter()
        .find(|col| col.name == "mixed")
        .expect("mixed column present");
    assert_eq!(mixed_column.datatype, ColumnType::Integer);
}

#[test]
fn probe_honors_input_encoding() {
    let dir = tempdir().expect("temp dir");
    let input_path = dir.path().join("encoded.csv");
    let schema_path = dir.path().join("encoded-schema.yml");

    let content = "id,name\n1,Caf\u{e9}\n";
    let (encoded, _, _) = WINDOWS_1252.encode(content);
    fs::write(&input_path, &encoded).expect("write encoded input");

    cargo_bin_cmd!("csv-managed")
        .args([
            "schema",
            "infer",
            "-i",
            input_path.to_str().unwrap(),
            "-o",
            schema_path.to_str().unwrap(),
            "--input-encoding",
            "windows-1252",
        ])
        .assert()
        .success();

    let schema = Schema::load(&schema_path).expect("load encoded schema");
    assert_eq!(schema.columns.len(), 2);
    assert_eq!(schema.columns[1].name, "name");
}
