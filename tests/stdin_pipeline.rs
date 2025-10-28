//! Tests for stdin ("-" sentinel) pipelines to ensure chaining works.
//!
//! These tests validate that:
//! 1. `process` can read from stdin using `-i -` and emit projected columns.
//! 2. Output from `process` can be piped into `stats` (simulated by capturing stdout and writing to stdin).
//! 3. Basic assertions confirm presence of expected headers / derived columns.
//!
//! NOTE: We rely on existing test fixtures in `tests/data`.

use assert_cmd::Command;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use tempfile::NamedTempFile;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("data")
        .join(name)
}

#[test]
fn process_reads_from_stdin_and_projects_columns() -> anyhow::Result<()> {
    let input = fixture("big_5_players_stats_2023_2024.csv");
    let schema = fixture("big_5_players_stats-schema.yml");
    let data = fs::read_to_string(&input)?;

    let assert = Command::cargo_bin("csv-managed")?
        .args([
            "process",
            "-i",
            "-", // stdin sentinel
            "--schema",
            schema.to_str().unwrap(),
            "--columns",
            "Player",
            "--columns",
            "Performance_Gls",
            "--limit",
            "3",
            "--table",
        ])
        .write_stdin(data)
        .assert()
        .success();

    let out = String::from_utf8(assert.get_output().stdout.clone())?;
    // With a schema, headers render using snake_case rename mappings.
    assert!(
        out.contains("player"),
        "Expected mapped header 'player' in table output"
    );
    assert!(
        out.contains("performance_gls"),
        "Expected mapped projected column 'performance_gls'"
    );
    // Table output has header row + at least 3 data rows
    let lines = out.lines().count();
    assert!(lines >= 4, "Expected >= 4 lines (header + 3 rows)");
    Ok(())
}

#[test]
fn chained_process_into_stats_via_memory_pipe() -> anyhow::Result<()> {
    // Use stats_schema dataset which has a Float column (price) suitable for stats.
    let input = fixture("stats_schema.csv");
    let schema = fixture("stats_schema-schema.yml");
    let raw = fs::read_to_string(&input)?;

    // Stage 1: process derive + projection
    let stage1 = Command::cargo_bin("csv-managed")?
        .args([
            "process",
            "-i",
            "-",
            "--schema",
            schema.to_str().unwrap(),
            "--columns",
            "id",
            "--columns",
            "quantity",
            "--columns",
            "price",
            "--columns",
            "status",
            "--limit",
            "25",
        ])
        .write_stdin(raw)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stage1_out = String::from_utf8(stage1.clone())?;
    assert!(
        stage1_out.contains("price"),
        "Expected numeric column present in stage 1 output"
    );

    // Stage 2: stats over streamed output
    let stats_run = Command::cargo_bin("csv-managed")?
        .args([
            "stats",
            "-i",
            "-",
            "--schema",
            schema.to_str().unwrap(),
            "-C",
            "price",
            "--limit",
            "200",
        ])
        .write_stdin(stage1)
        .assert()
        .success();

    let stats_out = String::from_utf8(stats_run.get_output().stdout.clone())?;
    assert!(
        stats_out.contains("price"),
        "Stats output should include the numeric column name"
    );
    assert!(
        stats_out.contains("count"),
        "Stats table should contain 'count' row or header"
    );
    Ok(())
}

#[test]
fn derived_column_before_stats_fails_header_mismatch() -> anyhow::Result<()> {
    let input = fixture("stats_schema.csv");
    let schema = fixture("stats_schema-schema.yml");
    let raw = fs::read_to_string(&input)?;

    let stage1 = Command::cargo_bin("csv-managed")?
        .args([
            "process",
            "-i",
            "-",
            "--schema",
            schema.to_str().unwrap(),
            "--derive",
            "extra=price*2",
            "--limit",
            "10",
        ])
        .write_stdin(raw)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stage1_text = String::from_utf8(stage1.clone())?;
    assert!(
        stage1_text.contains("extra"),
        "Derived column should be present in stage 1 output"
    );

    let stats = Command::cargo_bin("csv-managed")?
        .args([
            "stats",
            "-i",
            "-",
            "--schema",
            schema.to_str().unwrap(),
            "-C",
            "price",
        ])
        .write_stdin(stage1)
        .assert()
        .failure();

    let stderr = String::from_utf8(stats.get_output().stderr.clone())?;
    assert!(
        stderr.contains("Header length mismatch"),
        "Expected header mismatch error when schema does not reflect derived column"
    );
    Ok(())
}

#[test]
fn encoding_pipeline_process_to_stats_utf8_output() -> anyhow::Result<()> {
    let mut schema_file = NamedTempFile::new()?;
    writeln!(
        schema_file,
        "schema_version: 1.0\nhas_headers: true\ncolumns:\n  - name: id\n    datatype: Integer\n  - name: name\n    datatype: String"
    )?;
    schema_file.flush()?;

    let schema_path = schema_file.path().to_path_buf();
    let encoded: Vec<u8> = b"id,name\n1,Caf\xe9\n2,Ni\xf1o\n".to_vec();

    let stage1 = Command::cargo_bin("csv-managed")?
        .args([
            "process",
            "-i",
            "-",
            "--schema",
            schema_path.to_str().unwrap(),
            "--input-encoding",
            "windows-1252",
        ])
        .write_stdin(encoded)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stage1_text = String::from_utf8(stage1.clone())?;
    assert!(
        stage1_text.contains("Caf\u{00E9}"),
        "Process output should normalize Caf\u{00E9} to UTF-8"
    );
    assert!(
        stage1_text.contains("Ni\u{00F1}o"),
        "Process output should normalize Ni\u{00F1}o to UTF-8"
    );

    let stats = Command::cargo_bin("csv-managed")?
        .args([
            "stats",
            "-i",
            "-",
            "--schema",
            schema_path.to_str().unwrap(),
            "-C",
            "id",
        ])
        .write_stdin(stage1)
        .assert()
        .success();

    let stats_out = String::from_utf8(stats.get_output().stdout.clone())?;
    assert!(
        stats_out.contains("id"),
        "Stats output should reference the numeric column"
    );
    assert!(
        stats_out.contains("count"),
        "Stats output should include the count metric"
    );
    Ok(())
}

#[test]
#[ignore = "Pending schema evolution support for evolved layout chaining"]
fn encoding_pipeline_with_schema_evolution_pending() {
    // TODO: Implement once process can emit a derived schema for downstream typed stages.
}
