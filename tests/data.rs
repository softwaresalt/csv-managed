use std::str::FromStr;

use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use csv_managed::data::{
    normalize_column_name,
    parse_decimal_literal,
    parse_naive_date,
    parse_naive_datetime,
    parse_naive_time,
    parse_typed_value,
    value_to_evalexpr,
    ComparableValue,
    CurrencyValue,
    FixedDecimalValue,
    Value,
};
use csv_managed::schema::{ColumnType, DecimalSpec};
use evalexpr::Value as EvalValue;
use rust_decimal::Decimal;
use uuid::Uuid;

#[test]
fn normalize_column_name_replaces_non_alphanumeric() {
    assert_eq!(normalize_column_name("Order ID"), "order_id");
    assert_eq!(normalize_column_name("$Percent%"), "_percent_");
    assert_eq!(normalize_column_name("123Metric"), "_123metric");
    assert_eq!(normalize_column_name(""), "column");
}

#[test]
fn parse_naive_date_supports_multiple_formats() {
    let expected = NaiveDate::from_ymd_opt(2024, 5, 6).unwrap();
    assert_eq!(parse_naive_date("2024-05-06").unwrap(), expected);
    assert_eq!(parse_naive_date("06/05/2024").unwrap(), expected);
    assert_eq!(parse_naive_date("2024/05/06").unwrap(), expected);
}

#[test]
fn parse_naive_datetime_supports_multiple_formats() {
    let expected = NaiveDateTime::parse_from_str("2024-05-06 14:30:00", "%Y-%m-%d %H:%M:%S")
        .unwrap();
    assert_eq!(parse_naive_datetime("2024-05-06T14:30:00").unwrap(), expected);
    assert_eq!(parse_naive_datetime("06/05/2024 14:30:00").unwrap(), expected);
    assert_eq!(parse_naive_datetime("2024-05-06 14:30").unwrap(), expected);
}

#[test]
fn parse_naive_time_supports_multiple_formats() {
    let expected = NaiveTime::from_hms_opt(14, 30, 0).unwrap();
    assert_eq!(parse_naive_time("14:30:00").unwrap(), expected);
    assert_eq!(parse_naive_time("14:30").unwrap(), expected);
    assert!(parse_naive_time("24:61").is_err());
}

#[test]
fn parse_typed_value_handles_empty_and_boolean_inputs() {
    assert_eq!(parse_typed_value("", &ColumnType::Integer).unwrap(), None);

    let truthy = parse_typed_value("Yes", &ColumnType::Boolean)
        .unwrap()
        .unwrap();
    assert_eq!(truthy, Value::Boolean(true));

    let falsy = parse_typed_value("0", &ColumnType::Boolean)
        .unwrap()
        .unwrap();
    assert_eq!(falsy, Value::Boolean(false));

    assert!(parse_typed_value("maybe", &ColumnType::Boolean).is_err());
}

#[test]
fn parse_typed_value_supports_guid_inputs() {
    let raw = "550e8400-e29b-41d4-a716-446655440000";
    let parsed = parse_typed_value(raw, &ColumnType::Guid).unwrap().unwrap();
    match parsed {
        Value::Guid(g) => assert_eq!(g, Uuid::parse_str(raw).unwrap()),
        other => panic!("Expected GUID value, got {other:?}"),
    }

    let braced = "{550e8400-e29b-41d4-a716-446655440000}";
    let parsed_braced = parse_typed_value(braced, &ColumnType::Guid)
        .unwrap()
        .unwrap();
    assert!(matches!(parsed_braced, Value::Guid(_)));

    assert!(parse_typed_value("not-a-guid", &ColumnType::Guid).is_err());
}

#[test]
fn value_to_evalexpr_preserves_variants() {
    assert_eq!(value_to_evalexpr(&Value::Integer(42)), EvalValue::Int(42));
    assert_eq!(
        value_to_evalexpr(&Value::Boolean(false)),
        EvalValue::Boolean(false)
    );

    let date = NaiveDate::from_ymd_opt(2024, 5, 6).unwrap();
    assert_eq!(
        value_to_evalexpr(&Value::Date(date)),
        EvalValue::String("2024-05-06".to_string())
    );
}

#[test]
fn comparable_value_orders_none_before_some() {
    let none = ComparableValue(None);
    let some = ComparableValue(Some(Value::Integer(0)));
    assert!(none < some);
}

#[test]
fn parse_currency_values_accepts_two_and_four_decimals() {
    let two = parse_typed_value("$1,234.56", &ColumnType::Currency)
        .unwrap()
        .unwrap();
    let four = parse_typed_value("123.4567", &ColumnType::Currency)
        .unwrap()
        .unwrap();
    match (two, four) {
        (Value::Currency(a), Value::Currency(b)) => {
            assert_eq!(a.scale(), 2);
            assert_eq!(a.to_string_fixed(), "1234.56");
            assert_eq!(b.scale(), 4);
            assert_eq!(b.to_string_fixed(), "123.4567");
        }
        _ => panic!("Expected currency values"),
    }
}

#[test]
fn parse_currency_rejects_invalid_precision() {
    assert!(parse_typed_value("1.234", &ColumnType::Currency).is_err());
    assert!(parse_typed_value("abc", &ColumnType::Currency).is_err());
}

#[test]
fn parse_currency_rejects_embedded_letters() {
    let err = parse_typed_value("12a.34", &ColumnType::Currency)
        .expect_err("currency parser should reject embedded letters");
    assert!(err.to_string().contains("contains unsupported character"));
}

#[test]
fn currency_quantize_rounds_half_away_from_zero() {
    let decimal = Decimal::from_str("10.005").unwrap();
    let value = CurrencyValue::quantize(decimal, 2, None).expect("round currency");
    assert_eq!(value.to_string_fixed(), "10.01");
}

#[test]
fn currency_quantize_truncates_values() {
    let decimal = Decimal::from_str("7.899").unwrap();
    let value = CurrencyValue::quantize(decimal, 2, Some("truncate")).expect("truncate currency");
    assert_eq!(value.to_string_fixed(), "7.89");
}

#[test]
fn currency_quantize_truncates_four_decimal_precision() {
    let decimal = Decimal::from_str("1.234567").unwrap();
    let value = CurrencyValue::quantize(decimal, 4, Some("truncate")).expect("truncate currency");
    assert_eq!(value.to_string_fixed(), "1.2345");
}

#[test]
fn currency_quantize_rejects_invalid_strategy() {
    let decimal = Decimal::from_str("1.00").unwrap();
    assert!(CurrencyValue::quantize(decimal, 2, Some("ceil")).is_err());
}

#[test]
fn currency_quantize_rejects_invalid_scale() {
    let decimal = Decimal::from_str("1.00").unwrap();
    assert!(CurrencyValue::quantize(decimal, 3, None).is_err());
}

#[test]
fn currency_to_string_fixed_pads_fractional_zeros() {
    let value = CurrencyValue::parse("42").expect("parse integer currency");
    assert_eq!(value.to_string_fixed(), "42.00");
}

#[test]
fn fixed_decimal_value_truncate_strategy_respects_scale() {
    let spec = DecimalSpec::new(8, 2).expect("valid decimal spec");
    let decimal = Decimal::from_str("123.456").expect("valid decimal literal");
    let value = FixedDecimalValue::from_decimal(decimal, &spec, Some("truncate"))
        .expect("truncate decimal");
    assert_eq!(value.to_string_fixed(), "123.45");
    assert_eq!(value.scale(), 2);
}

#[test]
fn fixed_decimal_value_round_strategy_respects_scale() {
    let spec = DecimalSpec::new(10, 3).expect("valid decimal spec");
    let decimal = Decimal::from_str("-87.6549").expect("valid decimal literal");
    let value =
        FixedDecimalValue::from_decimal(decimal, &spec, Some("round")).expect("round decimal");
    assert_eq!(value.to_string_fixed(), "-87.655");
    assert_eq!(value.scale(), 3);
}

#[test]
fn fixed_decimal_value_rejects_precision_overflow() {
    let spec = DecimalSpec::new(6, 2).expect("valid decimal spec");
    let decimal = Decimal::from_str("12345.67").expect("decimal literal");
    let err =
        FixedDecimalValue::from_decimal(decimal, &spec, None).expect_err("precision overflow");
    assert!(err.to_string().contains("must not exceed"));
}

#[test]
fn fixed_decimal_value_rescales_short_fractional_parts() {
    let spec = DecimalSpec::new(12, 4).expect("valid decimal spec");
    let decimal = Decimal::from_str("42").expect("whole number decimal");
    let value = FixedDecimalValue::from_decimal(decimal, &spec, None).expect("rescale decimal");
    assert_eq!(value.to_string_fixed(), "42.0000");
    assert_eq!(value.scale(), 4);
}

#[test]
fn parse_decimal_literal_supports_parentheses_and_separators() {
    let parsed = parse_decimal_literal("(1,234.50)").expect("parse negative grouped decimal");
    assert_eq!(parsed, Decimal::from_str("-1234.50").unwrap());
}

#[test]
fn parse_decimal_literal_supports_positive_sign_and_underscores() {
    let parsed = parse_decimal_literal(" +7_654.321 ").expect("parse underscored decimal");
    assert_eq!(parsed, Decimal::from_str("7654.321").unwrap());
}

#[test]
fn parse_decimal_literal_rejects_invalid_characters() {
    assert!(parse_decimal_literal("12a.34").is_err());
    assert!(parse_decimal_literal("#42.0").is_err());
}

#[test]
fn parse_decimal_literal_rejects_multiple_decimal_points() {
    assert!(parse_decimal_literal("1.2.3").is_err());
}

#[test]
fn parse_decimal_values_enforce_precision_and_scale() {
    let spec = DecimalSpec::new(10, 4).expect("valid decimal spec");
    let decimal_type = ColumnType::Decimal(spec.clone());
    let parsed = parse_typed_value("123.4567", &decimal_type)
        .expect("parse decimal")
        .expect("non-empty decimal");
    match parsed {
        Value::Decimal(value) => {
            assert_eq!(value.scale(), 4);
            assert_eq!(value.precision(), 10);
            assert_eq!(value.to_string_fixed(), "123.4567");
        }
        other => panic!("Expected decimal value, got {other:?}"),
    }

    let narrow_spec = DecimalSpec::new(6, 2).expect("valid decimal spec");
    let narrow_type = ColumnType::Decimal(narrow_spec);
    assert!(parse_typed_value("123.456", &narrow_type).is_err());
    assert!(parse_typed_value("1234567", &narrow_type).is_err());
}
