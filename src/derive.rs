use anyhow::{Context, Result, anyhow};
use evalexpr::{Value as EvalValue, eval_with_context};
use std::str::FromStr;

use crate::{data::Value, expr, schema::ColumnType};

#[derive(Debug, Clone)]
pub struct DerivedColumn {
    pub name: String,
    pub expression: String,
    pub output_type: Option<ColumnType>,
}

impl DerivedColumn {
    pub fn parse(spec: &str) -> Result<Self> {
        let mut parts = spec.splitn(2, '=');
        let raw_name = parts
            .next()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .ok_or_else(|| anyhow!("Derived column is missing a name"))?;
        let (name, output_type) = if let Some((base, type_token)) = raw_name.split_once(':') {
            let column_name = base.trim();
            if column_name.is_empty() {
                return Err(anyhow!("Derived column name is empty"));
            }
            let column_type = ColumnType::from_str(type_token.trim()).with_context(|| {
                format!(
                    "Derived column '{}' has invalid datatype annotation '{}'",
                    column_name, type_token
                )
            })?;
            (column_name, Some(column_type))
        } else {
            (raw_name, None)
        };
        let expression = parts
            .next()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .ok_or_else(|| anyhow!("Derived column '{name}' is missing an expression"))?;
        Ok(DerivedColumn {
            name: name.to_string(),
            expression: expression.to_string(),
            output_type,
        })
    }

    pub fn evaluate(
        &self,
        headers: &[String],
        raw_row: &[String],
        typed_row: &[Option<Value>],
        row_number: Option<usize>,
    ) -> Result<String> {
        let context = expr::build_context(headers, raw_row, typed_row, row_number)?;

        let result = eval_with_context(&self.expression, &context)
            .with_context(|| format!("Evaluating expression for column '{}'", self.name))?;
        Ok(match result {
            EvalValue::String(s) => s,
            EvalValue::Int(i) => i.to_string(),
            EvalValue::Float(f) => f.to_string(),
            EvalValue::Boolean(b) => b.to_string(),
            EvalValue::Tuple(values) => values
                .into_iter()
                .map(|v| v.to_string())
                .collect::<Vec<_>>()
                .join("|"),
            EvalValue::Empty => String::new(),
        })
    }
}

pub fn parse_derived_columns(specs: &[String]) -> Result<Vec<DerivedColumn>> {
    specs
        .iter()
        .map(|spec| DerivedColumn::parse(spec))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_with_datatype_annotation() {
        let derived = DerivedColumn::parse("total:Integer=price + tax").unwrap();
        assert_eq!(derived.name, "total");
        assert_eq!(derived.expression, "price + tax");
        assert!(matches!(derived.output_type, Some(ColumnType::Integer)));
    }
}
