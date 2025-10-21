use anyhow::{Context, Result, anyhow};
use evalexpr::{Value as EvalValue, eval_with_context};

use crate::{data::Value, expr};

#[derive(Debug, Clone)]
pub struct DerivedColumn {
    pub name: String,
    pub expression: String,
}

impl DerivedColumn {
    pub fn parse(spec: &str) -> Result<Self> {
        let mut parts = spec.splitn(2, '=');
        let name = parts
            .next()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .ok_or_else(|| anyhow!("Derived column is missing a name"))?;
        let expression = parts
            .next()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .ok_or_else(|| anyhow!("Derived column '{name}' is missing an expression"))?;
        Ok(DerivedColumn {
            name: name.to_string(),
            expression: expression.to_string(),
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
