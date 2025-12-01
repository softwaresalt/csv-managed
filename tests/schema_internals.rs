use std::collections::BTreeMap;
use std::io::Write;
use std::str::FromStr;

use csv_managed::schema::{
    ColumnMeta,
    ColumnType,
    DatatypeMapping,
    DecimalSpec,
    PlaceholderPolicy,
    Schema,
    NumericKind,
    analyze_numeric_token,
    apply_placeholder_replacements,
    format_hint_for,
    infer_schema_with_stats,
};
use encoding_rs::UTF_8;
use proptest::prelude::*;
use serde_yaml::{self, Value};
use tempfile::NamedTempFile;

#[test]
fn infer_schema_with_stats_captures_samples() {
    let mut file = NamedTempFile::new().expect("temp file");
    writeln!(file, "id,date,value").unwrap();
    writeln!(file, "1,2024-01-01T08:30:00Z,$12.34").unwrap();
    writeln!(file, "2,2024-01-02T09:45:00Z,$56.78").unwrap();

    let policy = PlaceholderPolicy::default();
    let (schema, stats) = infer_schema_with_stats(file.path(), 0, b',', UTF_8, &policy, None)
        .expect("infer with stats");

    assert_eq!(schema.columns.len(), 3);
    assert_eq!(stats.sample_value(1), Some("2024-01-01T08:30:00Z"));
    assert_eq!(stats.sample_value(2), Some("$12.34"));
    assert_eq!(stats.rows_read(), 2);
    assert_eq!(stats.decode_errors(), 0);
}

#[test]
fn format_hint_detects_common_patterns() {
    let date_hint = format_hint_for(&ColumnType::Date, Some("2024/01/30"));
    assert_eq!(date_hint.as_deref(), Some("Slash-separated date"));

    let currency_hint = format_hint_for(&ColumnType::Float, Some("\u{20AC}1,234.50"));
    assert_eq!(currency_hint.as_deref(), Some("Currency symbol detected"));

    let guid_hint = format_hint_for(
        &ColumnType::Guid,
        Some("{ABCDEF12-3456-7890-ABCD-EF1234567890}"),
    );
    assert_eq!(guid_hint.as_deref(), Some("GUID with braces"));
}

#[test]
fn datatype_mappings_convert_string_to_date() {
    let mappings = vec![
        DatatypeMapping {
            from: ColumnType::String,
            to: ColumnType::DateTime,
            strategy: None,
            options: BTreeMap::new(),
        },
        DatatypeMapping {
            from: ColumnType::DateTime,
            to: ColumnType::Date,
            strategy: None,
            options: BTreeMap::new(),
        },
    ];

    let column = ColumnMeta {
        name: "event_date".to_string(),
        datatype: ColumnType::Date,
        rename: None,
        value_replacements: Vec::new(),
        datatype_mappings: mappings,
    };
    let schema = Schema {
        columns: vec![column],
        schema_version: None,
        has_headers: true,
    };

    let mut row = vec!["2024-05-10T13:45:00".to_string()];
    schema
        .apply_transformations_to_row(&mut row)
        .expect("apply datatype mappings");
    assert_eq!(row[0], "2024-05-10");
}

#[test]
fn datatype_mappings_round_float_values() {
    let mut options = BTreeMap::new();
    options.insert("scale".to_string(), Value::from(4));
    let mapping = DatatypeMapping {
        from: ColumnType::String,
        to: ColumnType::Float,
        strategy: Some("round".to_string()),
        options,
    };
    let column = ColumnMeta {
        name: "measurement".to_string(),
        datatype: ColumnType::Float,
        rename: None,
        value_replacements: Vec::new(),
        datatype_mappings: vec![mapping],
    };
    let schema = Schema {
        columns: vec![column],
        schema_version: None,
        has_headers: true,
    };
    let mut row = vec!["3.1415926535".to_string()];
    schema
        .apply_transformations_to_row(&mut row)
        .expect("round float");
    assert_eq!(row[0], "3.1416");
}

#[test]
fn datatype_mappings_round_currency_values() {
    let mut options = BTreeMap::new();
    options.insert("scale".to_string(), Value::from(2));
    let mapping = DatatypeMapping {
        from: ColumnType::String,
        to: ColumnType::Currency,
        strategy: Some("round".to_string()),
        options,
    };
    let column = ColumnMeta {
        name: "price".to_string(),
        datatype: ColumnType::Currency,
        rename: None,
        value_replacements: Vec::new(),
        datatype_mappings: vec![mapping],
    };
    let schema = Schema {
        columns: vec![column],
        schema_version: None,
        has_headers: true,
    };
    let mut row = vec!["12.345".to_string()];
    schema
        .apply_transformations_to_row(&mut row)
        .expect("round currency");
    assert_eq!(row[0], "12.35");
}

#[test]
fn datatype_mappings_preserve_currency_scale_when_unspecified() {
    let mapping = DatatypeMapping {
        from: ColumnType::String,
        to: ColumnType::Currency,
        strategy: None,
        options: BTreeMap::new(),
    };
    let column = ColumnMeta {
        name: "premium".to_string(),
        datatype: ColumnType::Currency,
        rename: None,
        value_replacements: Vec::new(),
        datatype_mappings: vec![mapping],
    };
    let schema = Schema {
        columns: vec![column],
        schema_version: None,
        has_headers: true,
    };
    let mut row = vec!["123.4567".to_string()];
    schema
        .apply_transformations_to_row(&mut row)
        .expect("preserve currency scale");
    assert_eq!(row[0], "123.4567");
}

#[test]
fn datatype_mappings_convert_currency_to_decimal() {
    let spec = DecimalSpec::new(10, 2).expect("decimal spec");
    let currency_mapping = DatatypeMapping {
        from: ColumnType::String,
        to: ColumnType::Currency,
        strategy: None,
        options: BTreeMap::new(),
    };
    let decimal_mapping = DatatypeMapping {
        from: ColumnType::Currency,
        to: ColumnType::Decimal(spec.clone()),
        strategy: Some("truncate".to_string()),
        options: BTreeMap::new(),
    };
    let column = ColumnMeta {
        name: "amount".to_string(),
        datatype: ColumnType::Decimal(spec.clone()),
        rename: None,
        value_replacements: Vec::new(),
        datatype_mappings: vec![currency_mapping, decimal_mapping],
    };
    let schema = Schema {
        columns: vec![column],
        schema_version: None,
        has_headers: true,
    };
    let mut row = vec!["$123.4567".to_string()];
    schema
        .apply_transformations_to_row(&mut row)
        .expect("currency to decimal mapping");
    assert_eq!(row[0], "123.45");
}

#[test]
fn infer_schema_identifies_currency_columns() {
    let mut file = NamedTempFile::new().expect("temp file");
    writeln!(file, "amount,name").unwrap();
    writeln!(file, "$12.34,alpha").unwrap();
    writeln!(file, "56.7800,beta").unwrap();

    let policy = PlaceholderPolicy::default();
    let (schema, _) = infer_schema_with_stats(file.path(), 0, b',', UTF_8, &policy, None)
        .expect("infer schema");
    assert_eq!(schema.columns.len(), 2);
    assert_eq!(schema.columns[0].datatype, ColumnType::Currency);
    assert_eq!(schema.columns[1].datatype, ColumnType::String);
}

#[test]
fn infer_schema_promotes_currency_when_symbol_ratio_met() {
    let mut file = NamedTempFile::new().expect("temp file");
    writeln!(file, "amount").unwrap();
    writeln!(file, "$12.00").unwrap();
    writeln!(file, "14").unwrap();
    writeln!(file, "15").unwrap();

    let policy = PlaceholderPolicy::default();
    let (schema, _) = infer_schema_with_stats(file.path(), 0, b',', UTF_8, &policy, None)
        .expect("infer schema");
    assert_eq!(schema.columns.len(), 1);
    assert_eq!(schema.columns[0].datatype, ColumnType::Currency);
}

#[test]
fn infer_schema_prefers_decimal_when_fraction_present() {
    let mut file = NamedTempFile::new().expect("temp file");
    writeln!(file, "amount").unwrap();
    writeln!(file, "1").unwrap();
    writeln!(file, "2").unwrap();
    writeln!(file, "3.5").unwrap();

    let policy = PlaceholderPolicy::default();
    let (schema, _) = infer_schema_with_stats(file.path(), 0, b',', UTF_8, &policy, None)
        .expect("infer schema");

    let expected = DecimalSpec::new(2, 1).expect("valid spec");
    match &schema.columns[0].datatype {
        ColumnType::Decimal(spec) => assert_eq!(spec, &expected),
        other => panic!("expected decimal column, got {other:?}"),
    }
}

#[test]
fn infer_schema_supports_scientific_notation_as_decimal() {
    let mut file = NamedTempFile::new().expect("temp file");
    writeln!(file, "value").unwrap();
    writeln!(file, "1e3").unwrap();
    writeln!(file, "2.5e-1").unwrap();

    let policy = PlaceholderPolicy::default();
    let (schema, _) = infer_schema_with_stats(file.path(), 0, b',', UTF_8, &policy, None)
        .expect("infer schema");

    let expected = DecimalSpec::new(6, 2).expect("valid spec");
    match &schema.columns[0].datatype {
        ColumnType::Decimal(spec) => assert_eq!(spec, &expected),
        other => panic!("expected decimal column, got {other:?}"),
    }
}

#[test]
fn infer_schema_treats_leading_zero_integers_as_string() {
    let mut file = NamedTempFile::new().expect("temp file");
    writeln!(file, "code").unwrap();
    writeln!(file, "001").unwrap();
    writeln!(file, "002").unwrap();
    writeln!(file, "003").unwrap();

    let policy = PlaceholderPolicy::default();
    let (schema, _) = infer_schema_with_stats(file.path(), 0, b',', UTF_8, &policy, None)
        .expect("infer schema");

    assert_eq!(schema.columns[0].datatype, ColumnType::String);
}

#[test]
fn infer_schema_prioritizes_decimal_over_currency_without_symbols() {
    let mut file = NamedTempFile::new().expect("temp file");
    writeln!(file, "amount").unwrap();
    writeln!(file, "12.34").unwrap();
    writeln!(file, "45.67").unwrap();

    let policy = PlaceholderPolicy::default();
    let (schema, _) = infer_schema_with_stats(file.path(), 0, b',', UTF_8, &policy, None)
        .expect("infer schema");

    let expected = DecimalSpec::new(4, 2).expect("valid spec");
    match &schema.columns[0].datatype {
        ColumnType::Decimal(spec) => assert_eq!(spec, &expected),
        other => panic!("expected decimal column, got {other:?}"),
    }
}

#[test]
fn analyze_numeric_token_handles_scientific_notation() {
    let observation = analyze_numeric_token("1e3")
        .expect("scientific notation should be recognized");
    assert!(matches!(observation.kind, NumericKind::Decimal));
}

#[test]
fn analyze_numeric_token_handles_scientific_with_fraction() {
    let observation = analyze_numeric_token("2.5e-1")
        .expect("scientific notation with fraction should be recognized");
    assert!(matches!(observation.kind, NumericKind::Decimal));
    assert_eq!(observation.scale, 2);
    assert_eq!(observation.precision, 2);
}

#[test]
fn infer_schema_prefers_majority_integer() {
    let mut file = NamedTempFile::new().expect("temp file");
    writeln!(file, "id,name").unwrap();
    writeln!(file, "1,alpha").unwrap();
    writeln!(file, "2,beta").unwrap();
    writeln!(file, "unknown,gamma").unwrap();

    let policy = PlaceholderPolicy::default();
    let (schema, _) = infer_schema_with_stats(file.path(), 0, b',', UTF_8, &policy, None)
        .expect("infer schema");
    assert_eq!(schema.columns[0].datatype, ColumnType::Integer);
    assert_eq!(schema.columns[1].datatype, ColumnType::String);
}

#[test]
fn infer_schema_prefers_majority_boolean() {
    let mut file = NamedTempFile::new().expect("temp file");
    writeln!(file, "flag").unwrap();
    writeln!(file, "true").unwrap();
    writeln!(file, "false").unwrap();
    writeln!(file, "unknown").unwrap();

    let policy = PlaceholderPolicy::default();
    let (schema, _) = infer_schema_with_stats(file.path(), 0, b',', UTF_8, &policy, None)
        .expect("infer schema");
    assert_eq!(schema.columns.len(), 1);
    assert_eq!(schema.columns[0].datatype, ColumnType::Boolean);
}

#[test]
fn infer_schema_collects_na_placeholders() {
    let mut file = NamedTempFile::new().expect("temp file");
    writeln!(file, "value").unwrap();
    writeln!(file, "NA").unwrap();
    writeln!(file, "#N/A").unwrap();
    writeln!(file, "42").unwrap();

    let policy = PlaceholderPolicy::default();
    let (_, stats) = infer_schema_with_stats(file.path(), 0, b',', UTF_8, &policy, None)
        .expect("infer stats");

    let summary = stats.placeholder_summary(0).expect("placeholder summary");
    let entries = summary.entries();
    assert_eq!(entries.len(), 2);
    assert!(
        entries
            .iter()
            .any(|(token, count)| token == "NA" && *count == 1)
    );
    assert!(
        entries
            .iter()
            .any(|(token, count)| token == "#N/A" && *count == 1)
    );
}

#[test]
fn assume_header_false_forces_field_names() {
    let mut file = NamedTempFile::new().expect("temp file");
    writeln!(file, "id,value").unwrap();
    writeln!(file, "1,2").unwrap();
    writeln!(file, "3,4").unwrap();

    let policy = PlaceholderPolicy::default();
    let (schema, _) = infer_schema_with_stats(file.path(), 0, b',', UTF_8, &policy, Some(false))
        .expect("force headerless schema");

    assert!(!schema.has_headers);
    let column_names: Vec<_> = schema.columns.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(column_names, vec!["field_0", "field_1"]);
}

#[test]
fn assume_header_true_preserves_first_row_names() {
    let mut file = NamedTempFile::new().expect("temp file");
    writeln!(file, "100,200").unwrap();
    writeln!(file, "1,2").unwrap();
    writeln!(file, "3,4").unwrap();

    let policy = PlaceholderPolicy::default();
    let (schema, stats) = infer_schema_with_stats(file.path(), 0, b',', UTF_8, &policy, Some(true))
        .expect("assume header true");

    assert!(schema.has_headers);
    let column_names: Vec<_> = schema.columns.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(column_names, vec!["100", "200"]);
    assert_eq!(stats.sample_value(0), Some("1"));
}

#[test]
fn apply_placeholder_replacements_respects_policy() {
    let mut file = NamedTempFile::new().expect("temp file");
    writeln!(file, "value").unwrap();
    writeln!(file, "NA").unwrap();
    writeln!(file, "#NA").unwrap();
    writeln!(file, "7").unwrap();

    let policy = PlaceholderPolicy::default();
    let (schema, stats) = infer_schema_with_stats(file.path(), 0, b',', UTF_8, &policy, None)
        .expect("infer schema");

    let mut schema_empty = schema.clone();
    let added_empty = apply_placeholder_replacements(&mut schema_empty, &stats, &policy);
    assert_eq!(added_empty, 2);
    assert!(
        schema_empty.columns[0]
            .value_replacements
            .iter()
            .any(|r| r.from == "NA" && r.to.is_empty())
    );
    assert!(
        schema_empty.columns[0]
            .value_replacements
            .iter()
            .any(|r| r.from == "#NA" && r.to.is_empty())
    );

    let mut schema_fill = schema.clone();
    let fill_policy = PlaceholderPolicy::FillWith("NULL".to_string());
    let added_fill = apply_placeholder_replacements(&mut schema_fill, &stats, &fill_policy);
    assert_eq!(added_fill, 2);
    assert!(
        schema_fill.columns[0]
            .value_replacements
            .iter()
            .all(|r| r.to == "NULL")
    );

    let added_duplicate = apply_placeholder_replacements(&mut schema_fill, &stats, &fill_policy);
    assert_eq!(added_duplicate, 0);
}

#[test]
fn parse_decimal_type_supports_positional_syntax() {
    let parsed = ColumnType::from_str("decimal(18,4)").expect("parse decimal positional");
    match parsed {
        ColumnType::Decimal(spec) => {
            assert_eq!(spec.precision, 18);
            assert_eq!(spec.scale, 4);
        }
        other => panic!("expected decimal column, got {other:?}"),
    }
}

#[test]
fn parse_decimal_type_supports_named_syntax() {
    let parsed = ColumnType::from_str("decimal(precision=20, scale=6)").expect("parse decimal named");
    let spec = parsed
        .decimal_spec()
        .expect("decimal spec present after parsing");
    assert_eq!(spec.precision, 20);
    assert_eq!(spec.scale, 6);
}

#[test]
fn parse_decimal_type_rejects_missing_scale() {
    let err = ColumnType::from_str("decimal(10)").expect_err("missing scale error");
    assert!(
        err.to_string()
            .contains("Decimal type requires a scale value")
    );
}

#[test]
fn schema_parsing_rejects_unsupported_structured_datatype() {
    let yaml = r#"
columns:
  - name: location
    datatype:
      geography: {}
"#;
    let err = serde_yaml::from_str::<Schema>(yaml)
        .expect_err("unsupported structured datatype should fail");
    assert!(
        err.to_string()
            .contains("Unsupported structured datatype 'geography'")
    );
}

#[test]
fn schema_parsing_rejects_decimal_precision_overflow() {
    let yaml = r#"
columns:
  - name: amount
    datatype: decimal(29,2)
"#;
    let err = serde_yaml::from_str::<Schema>(yaml).expect_err("precision overflow should fail");
    assert!(err.to_string().contains("Decimal precision must be <="));
}

#[test]
fn decimal_cli_token_formats_precision_and_scale() {
    let parsed = ColumnType::from_str("decimal(28,9)").expect("parse decimal for cli token");
    assert_eq!(parsed.cli_token(), "decimal(28,9)");
    assert_eq!(parsed.signature_token(), "decimal(28,9)");
    assert_eq!(parsed.describe(), "decimal(precision=28,scale=9)");
}

#[test]
fn datatype_mappings_convert_string_to_decimal_with_rounding() {
    let spec = DecimalSpec::new(12, 2).expect("valid decimal spec");
    let mapping = DatatypeMapping {
        from: ColumnType::String,
        to: ColumnType::Decimal(spec.clone()),
        strategy: Some("round".to_string()),
        options: BTreeMap::new(),
    };
    let column = ColumnMeta {
        name: "amount".to_string(),
        datatype: ColumnType::Decimal(spec.clone()),
        rename: None,
        value_replacements: Vec::new(),
        datatype_mappings: vec![mapping],
    };
    let schema = Schema {
        columns: vec![column],
        schema_version: None,
        has_headers: true,
    };
    let mut row = vec!["123.455".to_string()];
    schema
        .apply_transformations_to_row(&mut row)
        .expect("apply decimal rounding mapping");
    assert_eq!(row[0], "123.46");
}

#[test]
fn datatype_mappings_convert_string_to_decimal_with_truncation() {
    let spec = DecimalSpec::new(14, 3).expect("valid decimal spec");
    let mapping = DatatypeMapping {
        from: ColumnType::String,
        to: ColumnType::Decimal(spec.clone()),
        strategy: Some("truncate".to_string()),
        options: BTreeMap::new(),
    };
    let column = ColumnMeta {
        name: "measurement".to_string(),
        datatype: ColumnType::Decimal(spec.clone()),
        rename: None,
        value_replacements: Vec::new(),
        datatype_mappings: vec![mapping],
    };
    let schema = Schema {
        columns: vec![column],
        schema_version: None,
        has_headers: true,
    };
    let mut row = vec!["-87.6549".to_string()];
    schema
        .apply_transformations_to_row(&mut row)
        .expect("apply decimal truncation mapping");
    assert_eq!(row[0], "-87.654");
}

fn apply_grouping(value: &str, separator: char) -> String {
    let chars: Vec<char> = value.chars().collect();
    if chars.len() <= 3 {
        return value.to_string();
    }
    let mut grouped = String::new();
    let mut index = chars.len() % 3;
    if index == 0 {
        index = 3;
    }
    grouped.extend(&chars[..index]);
    while index < chars.len() {
        grouped.push(separator);
        grouped.extend(&chars[index..index + 3]);
        index += 3;
    }
    grouped
}

fn digit_strategy() -> impl Strategy<Value = char> {
    (0u8..=9).prop_map(|d| (b'0' + d) as char)
}

fn numeric_token_strategy() -> impl Strategy<Value = (String, u32, bool, bool)> {
        (
            1u64..=999_999,
            0u32..=4,
            any::<bool>(),
            any::<bool>(),
            any::<bool>(),
            prop_oneof![
                Just('$'),
                Just('\u{20AC}'), // Euro symbol
                Just('\u{00A3}'), // Pound symbol
                Just('\u{00A5}'), // Yen symbol
            ],
            proptest::option::of(prop_oneof![Just(','), Just('_'), Just(' ')]),
            any::<bool>(),
        )
        .prop_flat_map(
            |(
                integer,
                scale,
                negative,
                parentheses,
                use_symbol,
                symbol_char,
                separator,
                spaced,
            )| {
                let fraction_strategy = if scale == 0 {
                    Just(String::new()).boxed()
                } else {
                    proptest::collection::vec(digit_strategy(), scale as usize)
                        .prop_map(|digits| digits.into_iter().collect())
                        .boxed()
                };
                fraction_strategy.prop_map(move |fraction| {
                    let mut body = integer.to_string();
                    if let Some(sep) = separator {
                        body = apply_grouping(&body, sep);
                    }
                    if scale > 0 {
                        body.push('.');
                        body.push_str(&fraction);
                    }
                    let mut has_symbol = false;
                    if use_symbol {
                        has_symbol = true;
                        body = format!("{}{}", symbol_char, body);
                    }
                    let mut formatted = body;
                    let negative = negative && integer != 0;
                    let parentheses_active = parentheses && negative;
                    if negative {
                        if parentheses_active {
                            formatted = format!("({formatted})");
                        } else {
                            formatted = format!("-{formatted}");
                        }
                    }
                    if spaced {
                        formatted = format!(" {formatted} ");
                    }
                    (formatted, scale, has_symbol, parentheses_active)
                })
            },
        )
}

proptest! {
    #[test]
    fn analyze_numeric_token_handles_generated_numeric_formats(
        (token, scale, has_symbol, parentheses_active) in numeric_token_strategy()
    ) {
        let observation = analyze_numeric_token(&token)
            .expect("generated numeric token should classify");
        if scale > 0 {
            prop_assert_eq!(observation.kind, NumericKind::Decimal);
            prop_assert_eq!(observation.scale, scale);
        } else {
            prop_assert_eq!(observation.kind, NumericKind::Integer);
        }
        prop_assert_eq!(
            observation.has_currency_symbol,
            has_symbol || parentheses_active
        );
    }
}

#[test]
fn datatype_mappings_reject_unknown_currency_strategy() {
    let mut options = BTreeMap::new();
    options.insert("scale".to_string(), Value::from(2));
    let mapping = DatatypeMapping {
        from: ColumnType::String,
        to: ColumnType::Currency,
        strategy: Some("ceil".to_string()),
        options,
    };
    let column = ColumnMeta {
        name: "price".to_string(),
        datatype: ColumnType::Currency,
        rename: None,
        value_replacements: Vec::new(),
        datatype_mappings: vec![mapping],
    };
    let schema = Schema {
        columns: vec![column],
        schema_version: None,
        has_headers: true,
    };
    let mut row = vec!["12.34".to_string()];
    let err = schema
        .apply_transformations_to_row(&mut row)
        .expect_err("invalid currency strategy should fail");
    assert!(err.to_string().contains("Column 'price'"));
    assert!(err.chain().any(|source| {
        source
            .to_string()
            .contains("Unsupported currency rounding strategy")
    }));
}

#[test]
fn datatype_mappings_reject_decimal_precision_overflow() {
    let spec = DecimalSpec::new(8, 2).expect("decimal spec");
    let mapping = DatatypeMapping {
        from: ColumnType::String,
        to: ColumnType::Decimal(spec.clone()),
        strategy: None,
        options: BTreeMap::new(),
    };
    let column = ColumnMeta {
        name: "amount".to_string(),
        datatype: ColumnType::Decimal(spec.clone()),
        rename: None,
        value_replacements: Vec::new(),
        datatype_mappings: vec![mapping],
    };
    let schema = Schema {
        columns: vec![column],
        schema_version: None,
        has_headers: true,
    };
    let mut row = vec!["1234567.89".to_string()];
    let err = schema
        .apply_transformations_to_row(&mut row)
        .expect_err("precision overflow should fail");
    assert!(err.to_string().contains("Column 'amount'"));
    assert!(
        err.chain()
            .any(|source| source.to_string().contains("must not exceed"))
    );
}
