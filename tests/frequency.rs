use encoding_rs::UTF_8;

use csv_managed::{
    filter::FilterCondition,
    frequency::{compute_frequency_rows, FrequencyOptions},
    io_utils,
    schema,
};

const DATA_FILE: &str = "big_5_players_stats_2023_2024.csv";
const GOALS_COL: &str = "Performance_Gls";

fn fixture_path() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("data")
        .join(DATA_FILE)
}

#[test]
fn compute_frequency_rows_counts_goal_totals() {
    let path = fixture_path();
    assert!(path.exists(), "fixture missing: {path:?}");
    let delimiter = io_utils::resolve_input_delimiter(&path, None);
    let schema = schema::infer_schema(&path, 200, delimiter, UTF_8, None).expect("infer schema");
    let column_index = schema.column_index(GOALS_COL).expect("column index");
    let filter_storage: Vec<FilterCondition> = Vec::new();
    let expr_storage: Vec<String> = Vec::new();
    let options = FrequencyOptions {
        top: 3,
        row_limit: Some(100),
        filters: &filter_storage,
        filter_exprs: &expr_storage,
    };

    let rows = compute_frequency_rows(
        &path,
        &schema,
        delimiter,
        UTF_8,
        &[column_index],
        &options,
    )
    .expect("frequency rows");

    assert!(!rows.is_empty());
    assert_eq!(rows[0][0], GOALS_COL);
}
