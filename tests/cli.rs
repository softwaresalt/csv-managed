use std::{fs, io::Write};

use assert_cmd::Command;
use csv_managed::metadata::Schema;
use tempfile::tempdir;

fn write_sample_csv(delimiter: u8) -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempdir().expect("temp dir");
    let file_path = dir.path().join("sample.csv");
    let mut file = fs::File::create(&file_path).expect("create sample csv");
    writeln!(
        file,
        "id{}name{}amount{}status{}ordered_at",
        delimiter as char, delimiter as char, delimiter as char, delimiter as char
    )
    .unwrap();
    writeln!(
        file,
        "1{}Alice{}42.5{}shipped{}2024-01-01",
        delimiter as char, delimiter as char, delimiter as char, delimiter as char
    )
    .unwrap();
    writeln!(
        file,
        "2{}Bob{}13.37{}processing{}2024-01-03",
        delimiter as char, delimiter as char, delimiter as char, delimiter as char
    )
    .unwrap();
    (dir, file_path)
}

#[test]
fn probe_creates_metadata_with_custom_delimiter() {
    let (dir, csv_path) = write_sample_csv(b';');
    let meta_path = dir.path().join("schema.meta");
    Command::cargo_bin("csv-managed")
        .expect("binary exists")
        .args([
            "probe",
            "-i",
            csv_path.to_str().unwrap(),
            "-m",
            meta_path.to_str().unwrap(),
            "--delimiter",
            ";",
        ])
        .assert()
        .success();

    let contents = fs::read_to_string(&meta_path).expect("read meta");
    let schema: Schema = serde_json::from_str(&contents).expect("parse schema");
    assert_eq!(schema.columns.len(), 5);
    assert_eq!(schema.columns[0].name, "id");
}

#[test]
fn process_sorts_filters_and_derives_output() {
    let (dir, csv_path) = write_sample_csv(b',');
    let meta_path = dir.path().join("schema.meta");
    Command::cargo_bin("csv-managed")
        .expect("binary exists")
        .args([
            "probe",
            "-i",
            csv_path.to_str().unwrap(),
            "-m",
            meta_path.to_str().unwrap(),
        ])
        .assert()
        .success();

    let output_path = dir.path().join("filtered.csv");
    Command::cargo_bin("csv-managed")
        .expect("binary exists")
        .args([
            "process",
            "-i",
            csv_path.to_str().unwrap(),
            "-o",
            output_path.to_str().unwrap(),
            "--meta",
            meta_path.to_str().unwrap(),
            "--filter",
            "status = shipped",
            "--derive",
            "total_with_tax=amount*1.075",
            "--row-numbers",
        ])
        .assert()
        .success();

    let output = fs::read_to_string(&output_path).expect("read output");
    assert!(output.contains("row_number"));
    assert!(output.contains("total_with_tax"));
    assert!(output.lines().any(|line| line.contains("Alice")));
    assert!(!output.lines().any(|line| line.contains("Bob")));
}

#[test]
fn index_is_used_for_sorted_output() {
    let (dir, csv_path) = write_sample_csv(b',');
    let meta_path = dir.path().join("schema.meta");
    Command::cargo_bin("csv-managed")
        .expect("binary exists")
        .args([
            "probe",
            "-i",
            csv_path.to_str().unwrap(),
            "-m",
            meta_path.to_str().unwrap(),
        ])
        .assert()
        .success();

    let index_path = dir.path().join("data.idx");
    Command::cargo_bin("csv-managed")
        .expect("binary exists")
        .args([
            "index",
            "-i",
            csv_path.to_str().unwrap(),
            "-o",
            index_path.to_str().unwrap(),
            "-C",
            "ordered_at",
            "--meta",
            meta_path.to_str().unwrap(),
        ])
        .assert()
        .success();

    assert!(index_path.exists());

    let output_path = dir.path().join("sorted.csv");
    Command::cargo_bin("csv-managed")
        .expect("binary exists")
        .args([
            "process",
            "-i",
            csv_path.to_str().unwrap(),
            "-o",
            output_path.to_str().unwrap(),
            "--meta",
            meta_path.to_str().unwrap(),
            "--index",
            index_path.to_str().unwrap(),
            "--sort",
            "ordered_at:asc",
        ])
        .assert()
        .success();

    let output = fs::read_to_string(output_path).expect("read sorted");
    let mut lines = output.lines();
    let header = lines.next().expect("header");
    assert!(header.contains("ordered_at"));
    let first_row = lines.next().expect("first row");
    assert!(first_row.starts_with("1"));
}
