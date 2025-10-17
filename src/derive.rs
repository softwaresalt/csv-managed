use anyhow::{Context, Result, anyhow};
use evalexpr::{
    ContextWithMutableVariables, HashMapContext, Value as EvalValue, eval_with_context,
};

use crate::data::{Value, normalize_column_name, value_to_evalexpr};

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
        let mut context = HashMapContext::new();
        for (idx, header) in headers.iter().enumerate() {
            let canon = normalize_column_name(header);
            let key_by_index = format!("c{idx}");
            if let Some(value) = typed_row.get(idx).and_then(|v| v.as_ref()) {
                context
                    .set_value(canon.clone().into(), value_to_evalexpr(value))
                    .with_context(|| format!("Binding column '{header}'"))?;
                context
                    .set_value(key_by_index.clone().into(), value_to_evalexpr(value))
                    .with_context(|| format!("Binding column index {idx}"))?;
            } else if let Some(raw) = raw_row.get(idx) {
                context
                    .set_value(canon.clone().into(), EvalValue::String(raw.clone()))
                    .with_context(|| format!("Binding raw column '{header}'"))?;
                context
                    .set_value(key_by_index.clone().into(), EvalValue::String(raw.clone()))
                    .with_context(|| format!("Binding raw column index {idx}"))?;
            }
        }
        if let Some(row_number) = row_number {
            context
                .set_value("row_number".into(), EvalValue::Int(row_number as i64))
                .context("Binding row_number")?;
        }

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
