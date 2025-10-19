use std::collections::HashSet;
use std::str::FromStr;

use anyhow::{Context, Result, anyhow};
use log::info;

use crate::cli::SchemaArgs;
use crate::schema::{ColumnMeta, ColumnType, Schema, ValueReplacement};

pub fn execute(args: &SchemaArgs) -> Result<()> {
    let mut columns = parse_columns(&args.columns)
        .with_context(|| "Parsing --column definitions for schema creation".to_string())?;
    apply_replacements(&mut columns, &args.replacements)
        .with_context(|| "Parsing --replace definitions for schema creation".to_string())?;

    let schema = Schema { columns };
    schema
        .save(&args.output)
        .with_context(|| format!("Writing schema to {:?}", args.output))?;

    info!(
        "Defined schema with {} column(s) written to {:?}",
        schema.columns.len(),
        args.output
    );

    Ok(())
}

fn parse_columns(specs: &[String]) -> Result<Vec<ColumnMeta>> {
    let mut columns = Vec::new();
    let mut seen = HashSet::new();
    let mut output_names = HashSet::new();

    for raw in specs {
        for token in raw.split(',') {
            let token = token.trim();
            if token.is_empty() {
                continue;
            }
            let (name_part, type_part) = token.split_once(':').ok_or_else(|| {
                anyhow!("Column definition '{token}' must use the form name:type")
            })?;

            let name = name_part.trim();
            if name.is_empty() {
                return Err(anyhow!(
                    "Column name cannot be empty in definition '{token}'"
                ));
            }
            if !seen.insert(name.to_string()) {
                return Err(anyhow!("Duplicate column name '{name}' provided"));
            }

            let (type_raw, rename_raw) = if let Some((ty, rename)) = type_part.split_once("->") {
                (ty, Some(rename))
            } else {
                (type_part, None)
            };

            let column_type = ColumnType::from_str(type_raw.trim())
                .map_err(|err| anyhow!("Column '{name}' has invalid type '{type_part}': {err}"))?;

            let rename = rename_raw
                .map(|value| value.trim())
                .filter(|value| !value.is_empty())
                .map(|value| value.to_string());

            if let Some(ref alias) = rename {
                if alias != name && seen.contains(alias) {
                    return Err(anyhow!(
                        "Output name '{alias}' conflicts with an existing column name"
                    ));
                }
                if !output_names.insert(alias.clone()) {
                    return Err(anyhow!("Duplicate output column name '{alias}' provided"));
                }
            }

            if rename.is_none() {
                output_names.insert(name.to_string());
            }

            columns.push(ColumnMeta {
                name: name.to_string(),
                datatype: column_type,
                rename,
                value_replacements: Vec::new(),
            });
        }
    }

    if columns.is_empty() {
        return Err(anyhow!("At least one --column definition is required"));
    }

    Ok(columns)
}

fn apply_replacements(columns: &mut [ColumnMeta], specs: &[String]) -> Result<()> {
    if specs.is_empty() {
        return Ok(());
    }
    let mut lookup = HashSet::new();
    for column in columns.iter() {
        lookup.insert(column.name.clone());
    }

    for raw in specs {
        let spec = raw.trim();
        if spec.is_empty() {
            continue;
        }
        let (column_name, mapping) = spec.split_once('=').ok_or_else(|| {
            anyhow!("Replacement '{spec}' must use the form column=value->new_value")
        })?;
        let column_name = column_name.trim();
        if column_name.is_empty() {
            return Err(anyhow!("Replacement '{spec}' is missing a column name"));
        }
        if !lookup.contains(column_name) {
            return Err(anyhow!(
                "Replacement references unknown column '{column_name}'"
            ));
        }
        let (from_raw, to_raw) = mapping.split_once("->").ok_or_else(|| {
            anyhow!(
                "Replacement '{spec}' must include '->' to separate original and replacement values"
            )
        })?;
        let from = from_raw.trim().to_string();
        let to = to_raw.trim().to_string();
        let column = columns
            .iter_mut()
            .find(|c| c.name == column_name)
            .expect("column should exist");
        if let Some(existing) = column
            .value_replacements
            .iter()
            .position(|r| r.from == from)
        {
            column.value_replacements.remove(existing);
        }
        column
            .value_replacements
            .push(ValueReplacement { from, to });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_columns_accepts_comma_and_repeats() {
        let specs = vec![
            "id:integer,name:string".to_string(),
            "amount:float".to_string(),
        ];
        let columns = parse_columns(&specs).expect("parsed");
        assert_eq!(columns.len(), 3);
        assert_eq!(columns[0].name, "id");
        assert_eq!(columns[1].name, "name");
        assert_eq!(columns[2].name, "amount");
        assert_eq!(columns[0].datatype, ColumnType::Integer);
        assert_eq!(columns[1].datatype, ColumnType::String);
        assert_eq!(columns[2].datatype, ColumnType::Float);
    }

    #[test]
    fn duplicate_columns_are_rejected() {
        let specs = vec!["id:integer,id:string".to_string()];
        let err = parse_columns(&specs).unwrap_err();
        assert!(err.to_string().contains("Duplicate column name"));
    }

    #[test]
    fn missing_type_is_rejected() {
        let specs = vec!["id".to_string()];
        let err = parse_columns(&specs).unwrap_err();
        assert!(err.to_string().contains("must use the form"));
    }

    #[test]
    fn parse_columns_supports_output_rename() {
        let specs = vec!["id:integer->Identifier,name:string".to_string()];
        let columns = parse_columns(&specs).expect("parsed");
        assert_eq!(columns.len(), 2);
        assert_eq!(columns[0].rename.as_deref(), Some("Identifier"));
        assert!(columns[1].rename.is_none());
    }

    #[test]
    fn duplicate_output_names_are_rejected() {
        let specs = vec![
            "id:integer->Identifier".to_string(),
            "code:string->Identifier".to_string(),
        ];
        let err = parse_columns(&specs).unwrap_err();
        assert!(err.to_string().contains("Duplicate output column name"));
    }

    #[test]
    fn replacements_apply_to_columns() {
        let specs = vec!["status:string".to_string()];
        let mut columns = parse_columns(&specs).expect("parsed");
        let replacements = vec!["status=pending->shipped".to_string()];
        apply_replacements(&mut columns, &replacements).expect("applied");
        assert_eq!(columns[0].value_replacements.len(), 1);
        assert_eq!(columns[0].value_replacements[0].from, "pending");
        assert_eq!(columns[0].value_replacements[0].to, "shipped");
    }

    #[test]
    fn replacements_validate_column_names() {
        let specs = vec!["status:string".to_string()];
        let mut columns = parse_columns(&specs).expect("parsed");
        let replacements = vec!["missing=pending->shipped".to_string()];
        let err = apply_replacements(&mut columns, &replacements).unwrap_err();
        assert!(err.to_string().contains("unknown column"));
    }
}
