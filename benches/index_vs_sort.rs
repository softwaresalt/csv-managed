use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};

use criterion::{criterion_group, criterion_main, BatchSize, Criterion};
use csv_managed::cli::{BooleanFormat, ProcessArgs};
use csv_managed::index::{CsvIndex, IndexDefinition};
use csv_managed::process;
use encoding_rs::UTF_8;
use tempfile::TempDir;

fn generate_orders(rows: usize) -> (TempDir, PathBuf) {
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let csv_path = temp_dir.path().join("orders.csv");
    let mut file = File::create(&csv_path).expect("create csv");
    writeln!(file, "id,ordered_at,ship_time,status").expect("header");
    for i in 0..rows {
        let status = match i % 3 {
            0 => "shipped",
            1 => "pending",
            _ => "processing",
        };
        let day = (i % 28) + 1;
        let hour = (i % 23) + 1;
        writeln!(
            file,
            "{i},2024-01-{day:02},{hour:02}:00:00,{status}"
        )
        .expect("row");
    }
    (temp_dir, csv_path)
}

fn build_index(csv_path: &Path) -> PathBuf {
    let index_path = csv_path.parent().unwrap().join("orders.idx");
    let definitions = vec![
        IndexDefinition::parse("recent=ordered_at:desc").expect("parse recent"),
        IndexDefinition::parse("ordered_at:asc,ship_time:asc").expect("parse asc pair"),
    ];
    let index = CsvIndex::build(
        csv_path,
        &definitions,
        None,
        None,
        b',',
        UTF_8,
    )
    .expect("build index");
    index
        .save(&index_path)
        .expect("save index");
    index_path
}

fn base_process_args(input: &Path, output: &Path) -> ProcessArgs {
    ProcessArgs {
        input: input.to_path_buf(),
        output: Some(output.to_path_buf()),
        schema: None,
        index: None,
        index_variant: None,
        sort: vec!["ordered_at:asc".to_string(), "ship_time:asc".to_string()],
        columns: vec!["ordered_at".to_string(), "status".to_string()],
        exclude_columns: Vec::new(),
        derives: Vec::new(),
        filters: Vec::new(),
        filter_exprs: Vec::new(),
        row_numbers: false,
        limit: Some(20000),
        delimiter: None,
        output_delimiter: None,
        input_encoding: None,
        output_encoding: None,
        boolean_format: BooleanFormat::Original,
        preview: false,
        table: false,
        apply_mappings: false,
        skip_mappings: false,
    }
}

fn bench_index_vs_sort(c: &mut Criterion) {
    let (temp_dir, csv_path) = generate_orders(50_000);
    let index_path = build_index(csv_path.as_path());
    let in_memory_output = temp_dir.path().join("in_memory.csv");
    let indexed_output = temp_dir.path().join("indexed.csv");

    let in_memory_args = base_process_args(csv_path.as_path(), in_memory_output.as_path());
    let mut indexed_args = base_process_args(csv_path.as_path(), indexed_output.as_path());
    indexed_args.index = Some(index_path.clone());
    indexed_args.index_variant = Some("recent".to_string());
    indexed_args.sort = vec!["ordered_at:desc".to_string()];

    let mut group = c.benchmark_group("process_sort");

    group.bench_function("in_memory_sort", |b| {
        b.iter_batched(
            || (),
            |_| {
                process::execute(&in_memory_args).expect("process in-memory");
            },
            BatchSize::SmallInput,
        );
    });

    group.bench_function("indexed_sort_recent", |b| {
        b.iter_batched(
            || (),
            |_| {
                process::execute(&indexed_args).expect("process indexed");
            },
            BatchSize::SmallInput,
        );
    });

    drop(temp_dir);
    group.finish();
}

criterion_group!(benches, bench_index_vs_sort);
criterion_main!(benches);
