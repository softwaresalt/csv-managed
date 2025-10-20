use std::{env, fs, io::Write, process::Command as StdCommand};

use assert_cmd::Command;
use csv_managed::{
    index::CsvIndex,
    schema::{ColumnType, Schema, ValueReplacement},
};
use predicates::{prelude::PredicateBooleanExt, str::contains};
use serde_json::Value;
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
fn probe_creates_schema_with_custom_delimiter() {
    let (dir, csv_path) = write_sample_csv(b';');
    let schema_path = dir.path().join("schema.schema");
    Command::cargo_bin("csv-managed")
        .expect("binary exists")
        .args([
            "probe",
            "-i",
            csv_path.to_str().unwrap(),
            "-m",
            schema_path.to_str().unwrap(),
            "--delimiter",
            ";",
        ])
        .assert()
        .success();

    let contents = fs::read_to_string(&schema_path).expect("read schema");
    let json: Value = serde_json::from_str(&contents).expect("parse schema json");
    let columns = json["columns"].as_array().expect("columns array");
    assert!(columns.iter().all(|column| column.get("replace").is_none()));
    let schema: Schema = serde_json::from_str(&contents).expect("parse schema");
    assert_eq!(schema.columns.len(), 5);
    assert_eq!(schema.columns[0].name, "id");
}

#[test]
fn schema_command_writes_manual_schema() {
    let dir = tempdir().expect("temp dir");
    let schema_path = dir.path().join("manual.schema");

    Command::cargo_bin("csv-managed")
        .expect("binary exists")
        .args([
            "schema",
            "-o",
            schema_path.to_str().unwrap(),
            "-c",
            "id:integer->Identifier",
            "-c",
            "name:string->Customer Name,amount:float,guid:guid",
        ])
        .assert()
        .success();

    let contents = fs::read_to_string(&schema_path).expect("read manual schema");
    let json: Value = serde_json::from_str(&contents).expect("parse manual schema json");
    let columns = json["columns"].as_array().expect("columns array");
    assert!(columns.iter().all(|column| column.get("replace").is_none()));
    let schema: Schema = serde_json::from_str(&contents).expect("parse manual schema");
    assert_eq!(schema.columns.len(), 4);
    assert_eq!(schema.columns[0].name, "id");
    assert_eq!(schema.columns[0].rename.as_deref(), Some("Identifier"));
    assert_eq!(schema.columns[1].rename.as_deref(), Some("Customer Name"));
    assert_eq!(schema.columns[0].datatype, ColumnType::Integer);
    assert_eq!(schema.columns[1].datatype, ColumnType::String);
    assert_eq!(schema.columns[2].datatype, ColumnType::Float);
    assert_eq!(schema.columns[3].datatype, ColumnType::Guid);
}

#[test]
fn probe_includes_replace_template_when_requested() {
    let (dir, csv_path) = write_sample_csv(b',');
    let schema_path = dir.path().join("schema.schema");
    Command::cargo_bin("csv-managed")
        .expect("binary exists")
        .args([
            "probe",
            "-i",
            csv_path.to_str().unwrap(),
            "-m",
            schema_path.to_str().unwrap(),
            "--replace",
        ])
        .assert()
        .success();

    let contents = fs::read_to_string(&schema_path).expect("read schema");
    let json: Value = serde_json::from_str(&contents).expect("parse schema json");
    let columns = json["columns"].as_array().expect("columns array");
    assert!(columns.iter().all(|column| {
        column["replace"]
            .as_array()
            .map(|arr| arr.is_empty())
            .unwrap_or(false)
    }));
}

#[test]
fn columns_command_prints_schema_listing() {
    let (dir, csv_path) = write_sample_csv(b',');
    let schema_path = dir.path().join("schema.schema");
    Command::cargo_bin("csv-managed")
        .expect("binary exists")
        .args([
            "probe",
            "-i",
            csv_path.to_str().unwrap(),
            "-m",
            schema_path.to_str().unwrap(),
        ])
        .assert()
        .success();

    Command::cargo_bin("csv-managed")
        .expect("binary exists")
        .args(["columns", "--schema", schema_path.to_str().unwrap()])
        .assert()
        .success()
        .stdout(
            contains("name")
                .and(contains("type"))
                .and(contains("amount")),
        );
}

#[test]
fn probe_emits_mappings_into_schema_and_stdout() {
    let (dir, csv_path) = write_sample_csv(b',');
    let schema_path = dir.path().join("schema.schema");

    Command::cargo_bin("csv-managed")
        .expect("binary exists")
        .args([
            "probe",
            "-i",
            csv_path.to_str().unwrap(),
            "-m",
            schema_path.to_str().unwrap(),
            "--mapping",
        ])
        .assert()
        .success()
        .stdout(
            contains("mapping")
                .and(contains("id:integer->"))
                .and(contains("name:string->")),
        );

    let contents = fs::read_to_string(&schema_path).expect("schema output");
    let schema: Schema = serde_json::from_str(&contents).expect("parse schema");
    let mapping_values: Vec<Option<String>> = schema
        .columns
        .iter()
        .map(|col| col.rename.clone())
        .collect();
    assert_eq!(mapping_values[0].as_deref(), Some("id"));
    assert_eq!(mapping_values[1].as_deref(), Some("name"));
    assert_eq!(mapping_values[2].as_deref(), Some("amount"));
    assert_eq!(mapping_values[3].as_deref(), Some("status"));
    assert_eq!(mapping_values[4].as_deref(), Some("ordered_at"));
}

#[test]
fn process_sorts_filters_and_derives_output() {
    let (dir, csv_path) = write_sample_csv(b',');
    let schema_path = dir.path().join("schema.schema");
    Command::cargo_bin("csv-managed")
        .expect("binary exists")
        .args([
            "probe",
            "-i",
            csv_path.to_str().unwrap(),
            "-m",
            schema_path.to_str().unwrap(),
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
            "--schema",
            schema_path.to_str().unwrap(),
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
fn process_applies_value_replacements_from_schema() {
    let (dir, csv_path) = write_sample_csv(b',');
    let schema_path = dir.path().join("schema.schema");

    Command::cargo_bin("csv-managed")
        .expect("binary exists")
        .args([
            "probe",
            "-i",
            csv_path.to_str().unwrap(),
            "-m",
            schema_path.to_str().unwrap(),
        ])
        .assert()
        .success();

    let mut schema = Schema::load(&schema_path).expect("load probed schema");
    let status_column = schema
        .columns
        .iter_mut()
        .find(|col| col.name == "status")
        .expect("status column");
    status_column.value_replacements.push(ValueReplacement {
        from: "processing".to_string(),
        to: "pending".to_string(),
    });
    schema
        .save(&schema_path)
        .expect("save schema with replacements");

    let output_path = dir.path().join("replaced.csv");
    Command::cargo_bin("csv-managed")
        .expect("binary exists")
        .args([
            "process",
            "-i",
            csv_path.to_str().unwrap(),
            "-o",
            output_path.to_str().unwrap(),
            "--schema",
            schema_path.to_str().unwrap(),
        ])
        .assert()
        .success();

    let output = fs::read_to_string(&output_path).expect("read replaced output");
    assert!(output.contains("pending"));
    assert!(!output.contains(",processing"));
}

#[test]
fn verify_reports_header_mismatch_detail() {
    let (dir, csv_path) = write_sample_csv(b',');
    let schema_path = dir.path().join("schema.schema");

    Command::cargo_bin("csv-managed")
        .expect("binary exists")
        .args([
            "probe",
            "-i",
            csv_path.to_str().unwrap(),
            "-m",
            schema_path.to_str().unwrap(),
        ])
        .assert()
        .success();

    let bad_path = dir.path().join("bad.csv");
    fs::write(
        &bad_path,
        "id,wrong,amount,status,ordered_at\n1,Alice,42.5,shipped,2024-01-01\n2,Bob,13.37,processing,2024-01-03\n",
    )
    .expect("write bad csv");

    Command::cargo_bin("csv-managed")
        .expect("binary exists")
        .args([
            "verify",
            "-m",
            schema_path.to_str().unwrap(),
            "-i",
            bad_path.to_str().unwrap(),
        ])
        .assert()
        .failure()
        .stderr(
            contains("Validating headers for")
                .and(contains("Header mismatch"))
                .and(contains("expected 'name'"))
                .and(contains("wrong")),
        );
}

#[test]
fn index_is_used_for_sorted_output() {
    let (dir, csv_path) = write_sample_csv(b',');
    let schema_path = dir.path().join("schema.schema");
    Command::cargo_bin("csv-managed")
        .expect("binary exists")
        .args([
            "probe",
            "-i",
            csv_path.to_str().unwrap(),
            "-m",
            schema_path.to_str().unwrap(),
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
            "--schema",
            schema_path.to_str().unwrap(),
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
            "--schema",
            schema_path.to_str().unwrap(),
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

#[test]
fn index_combo_spec_generates_multiple_variants() {
    let (dir, csv_path) = write_sample_csv(b',');
    let schema_path = dir.path().join("schema.schema");
    Command::cargo_bin("csv-managed")
        .expect("binary exists")
        .args([
            "probe",
            "-i",
            csv_path.to_str().unwrap(),
            "-m",
            schema_path.to_str().unwrap(),
        ])
        .assert()
        .success();

    let index_path = dir.path().join("combo.idx");
    Command::cargo_bin("csv-managed")
        .expect("binary exists")
        .args([
            "index",
            "-i",
            csv_path.to_str().unwrap(),
            "-o",
            index_path.to_str().unwrap(),
            "--combo",
            "geo=ordered_at:asc|desc,amount:asc",
            "--schema",
            schema_path.to_str().unwrap(),
        ])
        .assert()
        .success();

    let index = CsvIndex::load(&index_path).expect("load combo index");
    assert!(index.variants().len() >= 4);
    assert!(
        index
            .variants()
            .iter()
            .filter_map(|variant| variant.name())
            .any(|name| name.starts_with("geo_"))
    );
}

#[test]
fn install_command_passes_arguments_to_cargo() {
    let dir = tempdir().expect("temp dir");
    let shim_src = dir.path().join("cargo_shim.rs");
    fs::write(
        &shim_src,
        r#"
        use std::{env, fs, path::PathBuf};

        fn main() {
            let log_path = env::var_os("CSV_MANAGED_TEST_LOG").expect("CSV_MANAGED_TEST_LOG");
            let joined = env::args().skip(1).collect::<Vec<_>>().join(" ");
            let path = PathBuf::from(log_path);
            fs::write(path, joined).expect("write log");
        }
        "#,
    )
    .expect("write cargo shim source");

    let shim_bin = dir
        .path()
        .join(format!("cargo-shim{}", env::consts::EXE_SUFFIX));
    let status = StdCommand::new("rustc")
        .arg(&shim_src)
        .arg("-O")
        .arg("-o")
        .arg(&shim_bin)
        .status()
        .expect("compile shim");
    assert!(status.success(), "failed to compile shim binary");

    let log_path = dir.path().join("captured_args.txt");
    let root_dir = dir.path().join("install-root");
    fs::create_dir_all(&root_dir).expect("create install root");

    let mut command = Command::cargo_bin("csv-managed").expect("binary exists");
    command
        .env("CSV_MANAGED_CARGO_SHIM", shim_bin.as_os_str())
        .env("CSV_MANAGED_TEST_LOG", log_path.as_os_str())
        .args([
            "install",
            "--version",
            "1.2.3",
            "--force",
            "--locked",
            "--root",
            root_dir.to_str().expect("root path"),
        ])
        .assert()
        .success();

    let captured = fs::read_to_string(&log_path).expect("read captured args");
    assert!(captured.contains("install"));
    assert!(captured.contains("csv-managed"));
    assert!(captured.contains("--version 1.2.3"));
    assert!(captured.contains("--force"));
    assert!(captured.contains("--locked"));
    assert!(captured.contains("--root"));
    assert!(captured.contains(root_dir.to_str().expect("root path")));
}

#[test]
fn process_accepts_named_index_variant() {
    let (dir, csv_path) = write_sample_csv(b',');
    let schema_path = dir.path().join("schema.schema");
    Command::cargo_bin("csv-managed")
        .expect("binary exists")
        .args([
            "probe",
            "-i",
            csv_path.to_str().unwrap(),
            "-m",
            schema_path.to_str().unwrap(),
        ])
        .assert()
        .success();

    let index_path = dir.path().join("multi.idx");
    Command::cargo_bin("csv-managed")
        .expect("binary exists")
        .args([
            "index",
            "-i",
            csv_path.to_str().unwrap(),
            "-o",
            index_path.to_str().unwrap(),
            "--spec",
            "default=ordered_at:asc",
            "--spec",
            "recent=ordered_at:desc",
            "--schema",
            schema_path.to_str().unwrap(),
        ])
        .assert()
        .success();

    let output_path = dir.path().join("recent.csv");
    Command::cargo_bin("csv-managed")
        .expect("binary exists")
        .args([
            "process",
            "-i",
            csv_path.to_str().unwrap(),
            "-o",
            output_path.to_str().unwrap(),
            "--schema",
            schema_path.to_str().unwrap(),
            "--index",
            index_path.to_str().unwrap(),
            "--index-variant",
            "recent",
            "--sort",
            "ordered_at:desc",
        ])
        .assert()
        .success();

    let output = fs::read_to_string(&output_path).expect("read recent output");
    let mut lines = output.lines();
    lines.next().expect("header");
    let first_row = lines.next().expect("first data row");
    assert!(first_row.starts_with("2"));
}

#[test]
fn process_errors_when_variant_missing() {
    let (dir, csv_path) = write_sample_csv(b',');
    let schema_path = dir.path().join("schema.schema");
    Command::cargo_bin("csv-managed")
        .expect("binary exists")
        .args([
            "probe",
            "-i",
            csv_path.to_str().unwrap(),
            "-m",
            schema_path.to_str().unwrap(),
        ])
        .assert()
        .success();

    let index_path = dir.path().join("single.idx");
    Command::cargo_bin("csv-managed")
        .expect("binary exists")
        .args([
            "index",
            "-i",
            csv_path.to_str().unwrap(),
            "-o",
            index_path.to_str().unwrap(),
            "--spec",
            "default=ordered_at:asc",
            "--schema",
            schema_path.to_str().unwrap(),
        ])
        .assert()
        .success();

    Command::cargo_bin("csv-managed")
        .expect("binary exists")
        .args([
            "process",
            "-i",
            csv_path.to_str().unwrap(),
            "--schema",
            schema_path.to_str().unwrap(),
            "--index",
            index_path.to_str().unwrap(),
            "--index-variant",
            "missing",
            "--sort",
            "ordered_at:asc",
        ])
        .assert()
        .failure()
        .stderr(contains("Index variant 'missing' not found"));
}
