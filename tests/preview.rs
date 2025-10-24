use std::{
    fs,
    path::{Path, PathBuf},
};

use assert_cmd::Command;
use csv::{ReaderBuilder, WriterBuilder};
use encoding_rs::WINDOWS_1252;
use predicates::str::contains;
use tempfile::{TempDir, tempdir};

const DATA_FILE: &str = "big_5_players_stats_2023_2024.csv";
const PLAYER_COLUMN: &str = "Player";
const DEFAULT_FIRST_PLAYER: &str = "Max Aarons";
const ROW_AFTER_DEFAULT_LIMIT: &str = "Zakaria Aboukhlal";
const FIFTH_ROW_PLAYER: &str = "Yunis Abdelhamid";
const PIPE_EXTENSION: &str = "csv";
const PREVIEW_COLUMNS: &[&str] = &["Rank", "Player", "Squad"];

struct DerivedFile {
    path: PathBuf,
    _dir: TempDir,
}

impl DerivedFile {
    fn path(&self) -> &Path {
        &self.path
    }
}

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("data")
        .join(name)
}

fn preview_subset(columns: &[&str], rows: usize) -> (Vec<String>, Vec<Vec<String>>) {
    let input = fixture_path(DATA_FILE);
    let mut reader = ReaderBuilder::new()
        .has_headers(true)
        .from_path(&input)
        .expect("open dataset");

    let headers = reader.headers().expect("headers").clone();
    let indices: Vec<usize> = columns
        .iter()
        .map(|name| {
            headers
                .iter()
                .position(|candidate| candidate == *name)
                .unwrap_or_else(|| panic!("Column '{name}' not found in dataset"))
        })
        .collect();

    let header_names = indices
        .iter()
        .map(|&idx| headers.get(idx).expect("header index").to_string())
        .collect::<Vec<String>>();

    let mut data_rows = Vec::new();
    for record in reader.records().take(rows) {
        let record = record.expect("record");
        let row = indices
            .iter()
            .map(|&idx| record.get(idx).unwrap_or("").to_string())
            .collect::<Vec<String>>();
        data_rows.push(row);
    }

    (header_names, data_rows)
}

fn write_subset_with_delimiter(
    columns: &[&str],
    rows: usize,
    delimiter: u8,
    extension: &str,
) -> DerivedFile {
    let (headers, data_rows) = preview_subset(columns, rows);
    let dir = tempdir().expect("temp dir for subset");
    let path = dir.path().join(format!("preview_subset.{extension}"));

    let mut writer = WriterBuilder::new()
        .delimiter(delimiter)
        .from_path(&path)
        .expect("create subset writer");
    writer
        .write_record(headers.iter())
        .expect("write subset headers");
    for row in &data_rows {
        writer.write_record(row.iter()).expect("write subset row");
    }
    writer.flush().expect("flush subset writer");

    DerivedFile { path, _dir: dir }
}

fn write_subset_windows1252(columns: &[&str], rows: usize) -> DerivedFile {
    let (headers, data_rows) = preview_subset(columns, rows);
    let dir = tempdir().expect("temp dir for encoded subset");
    let path = dir.path().join("preview_windows1252.csv");

    let mut writer = WriterBuilder::new()
        .delimiter(b',')
        .from_writer(Vec::<u8>::new());
    writer
        .write_record(headers.iter())
        .expect("write cp1252 headers");
    for row in &data_rows {
        writer.write_record(row.iter()).expect("write cp1252 row");
    }

    let utf8_bytes = writer.into_inner().expect("extract buffer");
    let utf8_string = String::from_utf8(utf8_bytes).expect("valid utf-8 subset");
    let (encoded, _, had_errors) = WINDOWS_1252.encode(&utf8_string);
    assert!(!had_errors, "failed to encode Windows-1252 sample");
    fs::write(&path, encoded.as_ref()).expect("write cp1252 file");

    DerivedFile { path, _dir: dir }
}

fn table_data_lines(rendered: &str) -> Vec<&str> {
    rendered
        .lines()
        .skip(2)
        .filter(|line| !line.trim().is_empty())
        .collect()
}

#[test]
fn preview_limits_to_default_row_count() {
    let input = fixture_path(DATA_FILE);
    let assert = Command::cargo_bin("csv-managed")
        .expect("binary exists")
        .args(["process", "-i", input.to_str().unwrap(), "--preview"])
        .assert()
        .success();

    let output = String::from_utf8(assert.get_output().stdout.clone()).expect("stdout");
    let data_lines = table_data_lines(&output);

    assert_eq!(
        data_lines.len(),
        10,
        "Expected preview to limit to 10 rows by default"
    );
    assert!(
        output
            .lines()
            .next()
            .unwrap_or_default()
            .contains(PLAYER_COLUMN)
    );
    assert!(
        data_lines
            .first()
            .unwrap_or(&"")
            .contains(DEFAULT_FIRST_PLAYER)
    );
    assert!(data_lines.iter().any(|line| line.contains("Abner")));
    assert!(
        !data_lines
            .iter()
            .any(|line| line.contains(ROW_AFTER_DEFAULT_LIMIT)),
        "Row 11 should be truncated"
    );
}

#[test]
fn preview_respects_rows_argument() {
    let input = fixture_path(DATA_FILE);
    let assert = Command::cargo_bin("csv-managed")
        .expect("binary exists")
        .args([
            "process",
            "-i",
            input.to_str().unwrap(),
            "--preview",
            "--limit",
            "5",
        ])
        .assert()
        .success();

    let output = String::from_utf8(assert.get_output().stdout.clone()).expect("stdout");
    let data_lines = table_data_lines(&output);

    assert_eq!(data_lines.len(), 5);
    assert!(data_lines[4].contains(FIFTH_ROW_PLAYER));
}

#[test]
fn preview_detects_tab_delimiter_from_extension() {
    let subset = write_subset_with_delimiter(PREVIEW_COLUMNS, 3, b'\t', "tsv");
    let assert = Command::cargo_bin("csv-managed")
        .expect("binary exists")
        .args([
            "process",
            "-i",
            subset.path().to_str().unwrap(),
            "--preview",
        ])
        .assert()
        .success();

    let output = String::from_utf8(assert.get_output().stdout.clone()).expect("stdout");
    let data_lines = table_data_lines(&output);

    assert!(
        output
            .lines()
            .next()
            .unwrap_or_default()
            .contains(PLAYER_COLUMN)
    );
    assert_eq!(data_lines.len(), 3);
    assert!(data_lines[1].contains("Brenden Aaronson"));
}

#[test]
fn preview_honors_explicit_delimiter() {
    let subset = write_subset_with_delimiter(PREVIEW_COLUMNS, 3, b'|', PIPE_EXTENSION);
    let assert = Command::cargo_bin("csv-managed")
        .expect("binary exists")
        .args([
            "process",
            "-i",
            subset.path().to_str().unwrap(),
            "--preview",
            "--delimiter",
            "|",
        ])
        .assert()
        .success();

    let output = String::from_utf8(assert.get_output().stdout.clone()).expect("stdout");
    let data_lines = table_data_lines(&output);

    assert!(output.lines().next().unwrap_or_default().contains("Rank"));
    assert_eq!(data_lines.len(), 3);
    assert!(data_lines[2].contains("Paxten Aaronson"));
}

#[test]
fn preview_decodes_using_provided_encoding() {
    let subset = write_subset_windows1252(PREVIEW_COLUMNS, 25);
    let assert = Command::cargo_bin("csv-managed")
        .expect("binary exists")
        .args([
            "process",
            "-i",
            subset.path().to_str().unwrap(),
            "--preview",
            "--input-encoding",
            "windows-1252",
            "--limit",
            "25",
        ])
        .assert()
        .success();

    let output = String::from_utf8(assert.get_output().stdout.clone()).expect("stdout");
    assert!(output.contains("Bénie Adama Traore"));
    assert!(!output.contains("BÃ©nie"));
}

#[test]
fn preview_with_output_fails() {
    let input = fixture_path(DATA_FILE);
    Command::cargo_bin("csv-managed")
        .expect("binary exists")
        .args([
            "process",
            "-i",
            input.to_str().unwrap(),
            "--preview",
            "-o",
            "ignored.csv",
        ])
        .assert()
        .failure()
        .stderr(contains("--preview cannot be combined with --output"));
}
