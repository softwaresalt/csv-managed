use anyhow::{Context, Result};
use chrono::{Duration, NaiveDate, NaiveDateTime, NaiveTime};
use evalexpr::{ContextWithMutableFunctions, ContextWithMutableVariables, eval_with_context, Function, HashMapContext, Value as EvalValue};

use crate::data::{
    normalize_column_name, parse_naive_date, parse_naive_datetime, parse_naive_time, value_to_evalexpr,
    Value,
};

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
        value if expected == 1 && !matches!(value, EvalValue::Tuple(_)) => {
            Ok(vec![value.clone()])
        }
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

pub fn build_context(
    headers: &[String],
    raw_row: &[String],
    typed_row: &[Option<Value>],
    row_number: Option<usize>,
) -> Result<HashMapContext> {
    let mut context = HashMapContext::new();
    register_temporal_functions(&mut context)?;
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

pub fn evaluate_expression_to_bool(expr: &str, context: &HashMapContext) -> Result<bool> {
    let result = eval_with_context(expr, context)
        .with_context(|| format!("Evaluating expression '{expr}'"))?;
    Ok(eval_value_truthy(result))
}

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
        let added = eval_with_context(
            "datetime_add_seconds(\"2024-01-01 00:00:00\", 3661)",
            &ctx,
        )
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
}
