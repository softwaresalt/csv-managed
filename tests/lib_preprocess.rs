use std::ffi::OsString;

use csv_managed::preprocess_cli_args_for_testing;
use proptest::prelude::*;

fn to_strings(args: &[OsString]) -> Vec<String> {
    args.iter()
        .map(|value| value.to_string_lossy().to_string())
        .collect()
}

#[test]
fn preprocess_cli_args_expands_report_invalid_segments() {
    let processed = preprocess_cli_args_for_testing(vec![
        OsString::from("csv-managed"),
        OsString::from("--report-invalid:stats:counts"),
        OsString::from("--dry-run"),
    ]);
    let tokens = to_strings(&processed);
    assert_eq!(
        tokens,
        vec![
            "csv-managed",
            "--report-invalid",
            "stats",
            "counts",
            "--dry-run",
        ]
    );
}

proptest! {
    #[test]
    fn preprocess_cli_args_splits_report_invalid_segments_prop(
        segments in proptest::collection::vec("[A-Za-z0-9_-]{1,8}", 1..5)
    ) {
        let mut arg = String::from("--report-invalid");
        for segment in &segments {
            arg.push(':');
            arg.push_str(segment);
        }
        let processed = preprocess_cli_args_for_testing(vec![
            OsString::from("csv-managed"),
            OsString::from(arg),
        ]);
        let tokens = to_strings(&processed);
        prop_assert_eq!(tokens[0].as_str(), "csv-managed");
        prop_assert_eq!(tokens[1].as_str(), "--report-invalid");
        for (idx, segment) in segments.iter().enumerate() {
            prop_assert_eq!(tokens[idx + 2].as_str(), segment.as_str());
        }
    }
}
