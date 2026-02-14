//! Expression engine for derived columns and filter expressions.
//!
//! Provides an [`evalexpr`]-based evaluation context with custom temporal helper
//! functions (FR-029), string functions (FR-030), built-in conditional logic
//! via `if(cond, then, else)` (FR-031), positional column aliases `c0`, `c1`, …
//! (FR-032), and optional `row_number` binding (FR-033).
//!
//! # Architecture
//!
//! * [`build_context`] constructs a per-row evaluation context with column
//!   values bound by canonical name and positional alias.
//! * [`evaluate_expression_to_bool`] evaluates a boolean expression for
//!   `--filter-expr`.
//! * [`eval_value_truthy`] converts an arbitrary eval value to a boolean.
//!
//! Shared by `--derive` and `--filter-expr` via `build_context()` in the
//! process pipeline.

use anyhow::{Context, Result};
use chrono::{Duration, NaiveDate, NaiveDateTime, NaiveTime};
use evalexpr::{
    ContextWithMutableFunctions, ContextWithMutableVariables, Function, HashMapContext,
    Value as EvalValue, eval_with_context,
};

use crate::data::{
    Value, normalize_column_name, parse_naive_date, parse_naive_datetime, parse_naive_time,
    value_to_evalexpr,
};

/// Register string helper functions into the evaluation context.
///
/// Currently registers:
/// - `concat(a, b, ...)` — concatenates arguments into a single string,
///   coercing non-string types to their display representation (FR-030).
fn register_string_functions(context: &mut HashMapContext) -> Result<()> {
    context
        .set_function(
            "concat".into(),
            Function::new(|arguments| {
                let parts = match arguments {
                    EvalValue::Tuple(values) => values.clone(),
                    EvalValue::Empty => Vec::new(),
                    single => vec![single.clone()],
                };
                let mut result = String::new();
                for part in &parts {
                    match part {
                        EvalValue::String(s) => result.push_str(s),
                        EvalValue::Int(i) => {
                            result.push_str(&format!("{i}"));
                        }
                        EvalValue::Float(f) => {
                            result.push_str(&format!("{f}"));
                        }
                        EvalValue::Boolean(b) => {
                            result.push_str(if *b { "true" } else { "false" });
                        }
                        EvalValue::Empty => {}
                        EvalValue::Tuple(_) => {
                            return Err(eval_error("concat does not accept nested tuples"));
                        }
                    }
                }
                Ok(EvalValue::String(result))
            }),
        )
        .map_err(anyhow::Error::from)?;
    Ok(())
}

/// Register all 11 temporal helper functions into the evaluation context
/// per FR-029: `date_add`, `date_sub`, `date_diff_days`, `date_format`,
/// `datetime_add_seconds`, `datetime_diff_seconds`, `datetime_format`,
/// `datetime_to_date`, `datetime_to_time`, `time_add_seconds`,
/// `time_diff_seconds`.
fn register_temporal_functions(context: &mut HashMapContext) -> Result<()> {
    context
        .set_function(
            "date_add".into(),
            Function::new(|arguments| {
                let args = expect_args(arguments, 2, "date_add")?;
                let date = parse_date_arg(&args[0])?;
                let days = parse_i64_arg(&args[1], "days")?;
                let result = date
                    .checked_add_signed(Duration::days(days))
                    .ok_or_else(|| eval_error("date_add overflow"))?;
                Ok(EvalValue::String(result.format("%Y-%m-%d").to_string()))
            }),
        )
        .map_err(anyhow::Error::from)?;

    context
        .set_function(
            "date_sub".into(),
            Function::new(|arguments| {
                let args = expect_args(arguments, 2, "date_sub")?;
                let date = parse_date_arg(&args[0])?;
                let days = parse_i64_arg(&args[1], "days")?;
                let result = date
                    .checked_sub_signed(Duration::days(days))
                    .ok_or_else(|| eval_error("date_sub overflow"))?;
                Ok(EvalValue::String(result.format("%Y-%m-%d").to_string()))
            }),
        )
        .map_err(anyhow::Error::from)?;

    context
        .set_function(
            "date_diff_days".into(),
            Function::new(|arguments| {
                let args = expect_args(arguments, 2, "date_diff_days")?;
                let end = parse_date_arg(&args[0])?;
                let start = parse_date_arg(&args[1])?;
                let diff = (end - start).num_days();
                Ok(EvalValue::Int(diff))
            }),
        )
        .map_err(anyhow::Error::from)?;

    context
        .set_function(
            "datetime_add_seconds".into(),
            Function::new(|arguments| {
                let args = expect_args(arguments, 2, "datetime_add_seconds")?;
                let dt = parse_datetime_arg(&args[0])?;
                let seconds = parse_i64_arg(&args[1], "seconds")?;
                let result = dt
                    .checked_add_signed(Duration::seconds(seconds))
                    .ok_or_else(|| eval_error("datetime_add_seconds overflow"))?;
                Ok(EvalValue::String(
                    result.format("%Y-%m-%d %H:%M:%S").to_string(),
                ))
            }),
        )
        .map_err(anyhow::Error::from)?;

    context
        .set_function(
            "datetime_diff_seconds".into(),
            Function::new(|arguments| {
                let args = expect_args(arguments, 2, "datetime_diff_seconds")?;
                let end = parse_datetime_arg(&args[0])?;
                let start = parse_datetime_arg(&args[1])?;
                Ok(EvalValue::Int((end - start).num_seconds()))
            }),
        )
        .map_err(anyhow::Error::from)?;

    context
        .set_function(
            "datetime_to_date".into(),
            Function::new(|arguments| {
                let args = expect_args(arguments, 1, "datetime_to_date")?;
                let dt = parse_datetime_arg(&args[0])?;
                Ok(EvalValue::String(dt.date().format("%Y-%m-%d").to_string()))
            }),
        )
        .map_err(anyhow::Error::from)?;

    context
        .set_function(
            "datetime_to_time".into(),
            Function::new(|arguments| {
                let args = expect_args(arguments, 1, "datetime_to_time")?;
                let dt = parse_datetime_arg(&args[0])?;
                Ok(EvalValue::String(dt.time().format("%H:%M:%S").to_string()))
            }),
        )
        .map_err(anyhow::Error::from)?;

    context
        .set_function(
            "time_add_seconds".into(),
            Function::new(|arguments| {
                let args = expect_args(arguments, 2, "time_add_seconds")?;
                let time = parse_time_arg(&args[0])?;
                let seconds = parse_i64_arg(&args[1], "seconds")?;
                let (result, overflow_days) =
                    time.overflowing_add_signed(Duration::seconds(seconds));
                if overflow_days != 0 {
                    return Err(eval_error("time_add_seconds overflow"));
                }
                Ok(EvalValue::String(result.format("%H:%M:%S").to_string()))
            }),
        )
        .map_err(anyhow::Error::from)?;

    context
        .set_function(
            "time_diff_seconds".into(),
            Function::new(|arguments| {
                let args = expect_args(arguments, 2, "time_diff_seconds")?;
                let end = parse_time_arg(&args[0])?;
                let start = parse_time_arg(&args[1])?;
                Ok(EvalValue::Int((end - start).num_seconds()))
            }),
        )
        .map_err(anyhow::Error::from)?;

    context
        .set_function(
            "date_format".into(),
            Function::new(|arguments| {
                let args = expect_args(arguments, 2, "date_format")?;
                let date = parse_date_arg(&args[0])?;
                let fmt = expect_string(&args[1], "format")?;
                Ok(EvalValue::String(date.format(fmt).to_string()))
            }),
        )
        .map_err(anyhow::Error::from)?;

    context
        .set_function(
            "datetime_format".into(),
            Function::new(|arguments| {
                let args = expect_args(arguments, 2, "datetime_format")?;
                let dt = parse_datetime_arg(&args[0])?;
                let fmt = expect_string(&args[1], "format")?;
                Ok(EvalValue::String(dt.format(fmt).to_string()))
            }),
        )
        .map_err(anyhow::Error::from)?;

    Ok(())
}

fn expect_args(
    arguments: &EvalValue,
    expected: usize,
    name: &str,
) -> Result<Vec<EvalValue>, evalexpr::EvalexprError> {
    match arguments {
        EvalValue::Empty if expected == 0 => Ok(Vec::new()),
        value if expected == 1 && !matches!(value, EvalValue::Tuple(_)) => Ok(vec![value.clone()]),
        EvalValue::Tuple(values) => {
            if values.len() != expected {
                return Err(evalexpr::EvalexprError::wrong_function_argument_amount(
                    values.len(),
                    expected,
                ));
            }
            Ok(values.clone())
        }
        _ => Err(eval_error(&format!(
            "{name} expects {expected} arguments provided as a tuple"
        ))),
    }
}

fn eval_error(message: &str) -> evalexpr::EvalexprError {
    evalexpr::EvalexprError::CustomMessage(message.to_string())
}

fn parse_date_arg(value: &EvalValue) -> Result<NaiveDate, evalexpr::EvalexprError> {
    let raw = expect_string(value, "date")?;
    parse_naive_date(raw).map_err(|err| eval_error(&err.to_string()))
}

fn parse_datetime_arg(value: &EvalValue) -> Result<NaiveDateTime, evalexpr::EvalexprError> {
    let raw = expect_string(value, "datetime")?;
    parse_naive_datetime(raw).map_err(|err| eval_error(&err.to_string()))
}

fn parse_time_arg(value: &EvalValue) -> Result<NaiveTime, evalexpr::EvalexprError> {
    let raw = expect_string(value, "time")?;
    parse_naive_time(raw).map_err(|err| eval_error(&err.to_string()))
}

fn parse_i64_arg(value: &EvalValue, name: &str) -> Result<i64, evalexpr::EvalexprError> {
    match value {
        EvalValue::Int(i) => Ok(*i),
        EvalValue::Float(f) => Ok(*f as i64),
        other => Err(eval_error(&format!(
            "Expected integer for {name}, got {other:?}",
        ))),
    }
}

fn expect_string<'a>(value: &'a EvalValue, name: &str) -> Result<&'a str, evalexpr::EvalexprError> {
    if let EvalValue::String(s) = value {
        Ok(s)
    } else {
        Err(eval_error(&format!("Expected string for {name}")))
    }
}

/// Build a per-row evaluation context for expression evaluation.
///
/// Binds each column by its canonical (snake_case) name and by positional
/// alias (`c0`, `c1`, …) per FR-032. When `row_number` is `Some`, binds
/// the `row_number` variable per FR-033. Registers temporal (FR-029) and
/// string (FR-030) helper functions.
///
/// # Complexity
///
/// O(n) where n is the number of columns.
pub fn build_context(
    headers: &[String],
    raw_row: &[String],
    typed_row: &[Option<Value>],
    row_number: Option<usize>,
) -> Result<HashMapContext> {
    let mut context = HashMapContext::new();
    register_temporal_functions(&mut context)?;
    register_string_functions(&mut context)?;
    for (idx, header) in headers.iter().enumerate() {
        let canon = normalize_column_name(header);
        let key = format!("c{idx}");
        if let Some(Some(value)) = typed_row.get(idx) {
            let eval_value = value_to_evalexpr(value);
            context
                .set_value(canon.clone(), eval_value.clone())
                .with_context(|| format!("Binding column '{header}'"))?;
            context
                .set_value(key, eval_value)
                .with_context(|| format!("Binding column index {idx}"))?;
        } else if let Some(raw) = raw_row.get(idx) {
            context
                .set_value(canon.clone(), EvalValue::String(raw.clone()))
                .with_context(|| format!("Binding raw column '{header}'"))?;
            context
                .set_value(key, EvalValue::String(raw.clone()))
                .with_context(|| format!("Binding raw column index {idx}"))?;
        }
    }

    if let Some(number) = row_number {
        context
            .set_value("row_number".to_string(), EvalValue::Int(number as i64))
            .context("Binding row_number")?;
    }

    Ok(context)
}

/// Evaluate a string expression against the given context and return a boolean.
///
/// Used by `--filter-expr` to determine row inclusion.
pub fn evaluate_expression_to_bool(expr: &str, context: &HashMapContext) -> Result<bool> {
    let result = eval_with_context(expr, context)
        .with_context(|| format!("Evaluating expression '{expr}'"))?;
    Ok(eval_value_truthy(result))
}

/// Convert an eval value to a boolean using truthy semantics.
///
/// - `Boolean(b)` → b
/// - `Int(i)` → i ≠ 0
/// - `Float(f)` → f ≠ 0.0
/// - `String(s)` → !s.is_empty()
/// - `Tuple(vs)` → any element is truthy
/// - `Empty` → false
pub fn eval_value_truthy(value: EvalValue) -> bool {
    match value {
        EvalValue::Boolean(b) => b,
        EvalValue::Int(i) => i != 0,
        EvalValue::Float(f) => f != 0.0,
        EvalValue::String(s) => !s.is_empty(),
        EvalValue::Tuple(values) => values.into_iter().any(eval_value_truthy),
        EvalValue::Empty => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn date_add_and_diff_work() {
        let mut ctx = HashMapContext::new();
        register_temporal_functions(&mut ctx).unwrap();
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
        let mut ctx = HashMapContext::new();
        register_temporal_functions(&mut ctx).unwrap();
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
        let mut ctx = HashMapContext::new();
        register_temporal_functions(&mut ctx).unwrap();
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
    fn concat_joins_strings() {
        let mut ctx = HashMapContext::new();
        register_string_functions(&mut ctx).unwrap();
        let result = eval_with_context("concat(\"hello\", \" \", \"world\")", &ctx)
            .unwrap()
            .as_string()
            .unwrap()
            .to_string();
        assert_eq!(result, "hello world");
    }

    #[test]
    fn concat_coerces_non_string_types() {
        let mut ctx = HashMapContext::new();
        register_string_functions(&mut ctx).unwrap();
        ctx.set_value("num".to_string(), EvalValue::Int(42))
            .unwrap();
        let result = eval_with_context("concat(\"value=\", num)", &ctx)
            .unwrap()
            .as_string()
            .unwrap()
            .to_string();
        assert_eq!(result, "value=42");
    }

    #[test]
    fn concat_single_argument() {
        let mut ctx = HashMapContext::new();
        register_string_functions(&mut ctx).unwrap();
        let result = eval_with_context("concat(\"solo\")", &ctx)
            .unwrap()
            .as_string()
            .unwrap()
            .to_string();
        assert_eq!(result, "solo");
    }

    #[test]
    fn if_function_selects_branch() {
        let ctx: HashMapContext = HashMapContext::new();
        let result_true = eval_with_context("if(true, \"yes\", \"no\")", &ctx)
            .unwrap()
            .as_string()
            .unwrap()
            .to_string();
        assert_eq!(result_true, "yes");
        let result_false = eval_with_context("if(false, \"yes\", \"no\")", &ctx)
            .unwrap()
            .as_string()
            .unwrap()
            .to_string();
        assert_eq!(result_false, "no");
    }

    #[test]
    fn if_function_with_comparison() {
        let mut ctx: HashMapContext = HashMapContext::new();
        ctx.set_value("amount".to_string(), EvalValue::Int(150))
            .unwrap();
        let result = eval_with_context("if(amount > 100, \"high\", \"low\")", &ctx)
            .unwrap()
            .as_string()
            .unwrap()
            .to_string();
        assert_eq!(result, "high");
    }

    #[test]
    fn row_number_available_in_context() {
        let headers = vec!["col_a".to_string()];
        let raw = vec!["value".to_string()];
        let typed = vec![Some(Value::String("value".to_string()))];
        let ctx = build_context(&headers, &raw, &typed, Some(42)).unwrap();
        let result = eval_with_context("row_number", &ctx)
            .unwrap()
            .as_int()
            .unwrap();
        assert_eq!(result, 42);
    }

    #[test]
    fn row_number_absent_without_flag() {
        let headers = vec!["col_a".to_string()];
        let raw = vec!["value".to_string()];
        let typed = vec![Some(Value::String("value".to_string()))];
        let ctx = build_context(&headers, &raw, &typed, None).unwrap();
        let result = eval_with_context("row_number", &ctx);
        assert!(
            result.is_err(),
            "row_number should not be bound when not enabled"
        );
    }

    #[test]
    fn positional_aliases_resolve_to_column_values() {
        let headers = vec!["first_name".to_string(), "last_name".to_string()];
        let raw = vec!["Alice".to_string(), "Smith".to_string()];
        let typed = vec![
            Some(Value::String("Alice".to_string())),
            Some(Value::String("Smith".to_string())),
        ];
        let ctx = build_context(&headers, &raw, &typed, None).unwrap();
        let c0 = eval_with_context("c0", &ctx)
            .unwrap()
            .as_string()
            .unwrap()
            .to_string();
        let c1 = eval_with_context("c1", &ctx)
            .unwrap()
            .as_string()
            .unwrap()
            .to_string();
        assert_eq!(c0, "Alice");
        assert_eq!(c1, "Smith");
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
}
