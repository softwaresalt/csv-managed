use csv_managed::data::{normalize_column_name, Value};
use csv_managed::expr::{build_context, evaluate_expression_to_bool};
use evalexpr::{eval_with_context, HashMapContext};
use proptest::prelude::*;

fn empty_context() -> HashMapContext {
    build_context(&[], &[], &[], None).expect("context with registered functions")
}

#[test]
fn date_add_and_diff_work() {
    let ctx = empty_context();
    let added = eval_with_context("date_add(\"2024-01-01\", 5)", &ctx)
        .unwrap()
        .as_string()
        .unwrap()
        .to_string();
    assert_eq!(added, "2024-01-06");

    let diff = eval_with_context("date_diff_days(\"2024-01-10\", \"2024-01-01\")", &ctx)
        .unwrap()
        .as_int()
        .unwrap();
    assert_eq!(diff, 9);
}

#[test]
fn datetime_functions_roundtrip() {
    let ctx = empty_context();
    let added = eval_with_context("datetime_add_seconds(\"2024-01-01 00:00:00\", 3661)", &ctx)
        .unwrap()
        .as_string()
        .unwrap()
        .to_string();
    assert_eq!(added, "2024-01-01 01:01:01");

    let diff = eval_with_context(
        "datetime_diff_seconds(\"2024-01-01 01:01:01\", \"2024-01-01 00:00:00\")",
        &ctx,
    )
    .unwrap()
    .as_int()
    .unwrap();
    assert_eq!(diff, 3661);
}

#[test]
fn time_functions_behave() {
    let ctx = empty_context();
    let added = eval_with_context("time_add_seconds(\"08:00:00\", 90)", &ctx)
        .unwrap()
        .as_string()
        .unwrap()
        .to_string();
    assert_eq!(added, "08:01:30");

    let diff = eval_with_context("time_diff_seconds(\"08:01:30\", \"08:00:00\")", &ctx)
        .unwrap()
        .as_int()
        .unwrap();
    assert_eq!(diff, 90);
}

#[test]
fn string_functions_cover_core_cases() {
    let ctx = empty_context();
    let lower = eval_with_context("lowercase(\"FOO\")", &ctx)
        .unwrap()
        .as_string()
        .unwrap()
        .to_string();
    assert_eq!(lower, "foo");

    let snake = eval_with_context("snake_case(\"Foo Bar\")", &ctx)
        .unwrap()
        .as_string()
        .unwrap()
        .to_string();
    assert_eq!(snake, "foo_bar");

    let trimmed = eval_with_context("trim(\"  value  \")", &ctx)
        .unwrap()
        .as_string()
        .unwrap()
        .to_string();
    assert_eq!(trimmed, "value");

    let substring = eval_with_context("substring(\"abcdef\", 1, 3)", &ctx)
        .unwrap()
        .as_string()
        .unwrap()
        .to_string();
    assert_eq!(substring, "bcd");

    let regex = eval_with_context("regex_replace(\"abc123\", \"[a-z]+\", \"X\")", &ctx)
        .unwrap()
        .as_string()
        .unwrap()
        .to_string();
    assert_eq!(regex, "X123");

    let camel = eval_with_context("camel_case(\"foo-bar baz\")", &ctx)
        .unwrap()
        .as_string()
        .unwrap()
        .to_string();
    assert_eq!(camel, "fooBarBaz");

    let pascal = eval_with_context("pascal_case(\"HTTP_STATUS\")", &ctx)
        .unwrap()
        .as_string()
        .unwrap()
        .to_string();
    assert_eq!(pascal, "HttpStatus");
}

proptest! {
    #[test]
    fn evaluate_expression_handles_random_numeric_context(
        a in -10_000i64..=10_000,
        b in -10_000i64..=10_000,
        header0 in "[A-Za-z0-9_ ]{3,12}",
        header1 in "[A-Za-z0-9_ ]{3,12}"
    ) {
        let headers = vec![header0.clone(), header1.clone()];
        let raw = vec![a.to_string(), b.to_string()];
        let typed = vec![Some(Value::Integer(a)), Some(Value::Integer(b))];
        let context = build_context(&headers, &raw, &typed, None).expect("build context");
        let name0 = normalize_column_name(&header0);
        let name1 = normalize_column_name(&header1);
        let expr_named = format!("({name0} + {name1}) > {name0}");
        let expr_indexed = "(c0 + c1) > c0";
        let lhs = evaluate_expression_to_bool(&expr_named, &context).expect("named expression");
        let rhs = evaluate_expression_to_bool(expr_indexed, &context).expect("indexed expression");
        prop_assert_eq!(lhs, rhs);
    }
}
