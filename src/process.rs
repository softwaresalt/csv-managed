use std::{fs::File, io::BufWriter, path::Path};

use anyhow::{Context, Result, anyhow};
use csv::{ByteRecord, Position, ReaderBuilder, WriterBuilder};
use itertools::Itertools;
use log::{debug, info};

use crate::{
    cli::ProcessArgs,
    data::{ComparableValue, Value, parse_typed_value},
    derive::{DerivedColumn, parse_derived_columns},
    filter::{evaluate_conditions, parse_filters},
    index::CsvIndex,
    metadata::{ColumnType, Schema},
};

pub fn execute(args: &ProcessArgs) -> Result<()> {
    let output_delimiter = args.output_delimiter.unwrap_or(args.delimiter);
    info!(
        "Processing '{}' -> {:?} (delimiter '{}', output '{}')",
        args.input.display(),
        args.output
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "stdout".into()),
        crate::printable_delimiter(args.delimiter),
        crate::printable_delimiter(output_delimiter)
    );
    let sorts = args
        .sort
        .iter()
        .flat_map(|s| s.split(','))
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(SortDirective::parse)
        .collect::<Result<Vec<_>>>()?;
    let selected_columns = args
        .columns
        .iter()
        .flat_map(|s| s.split(','))
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect::<Vec<_>>();
    let derived_columns = parse_derived_columns(&args.derives)?;
    let filters = parse_filters(&args.filters)?;

    let mut reader = ReaderBuilder::new()
        .has_headers(true)
        .delimiter(args.delimiter)
        .from_path(&args.input)
        .with_context(|| format!("Opening CSV file {:?}", args.input))?;
    let headers_record = reader.headers()?.clone();
    let headers: Vec<String> = headers_record.iter().map(|s| s.to_string()).collect();

    let mut schema = if let Some(meta_path) = &args.meta {
        Schema::load(meta_path)?
    } else {
        Schema::from_headers(&headers_record)
    };

    reconcile_schema_with_headers(&mut schema, &headers);

    let maybe_index = if let Some(index_path) = &args.index {
        Some(CsvIndex::load(index_path)?)
    } else {
        None
    };

    let sort_signature = sorts
        .iter()
        .map(|s| (s.column.clone(), s.ascending))
        .collect_vec();
    let supports_index = maybe_index
        .as_ref()
        .map(|index| index.supports_sort(&sort_signature))
        .unwrap_or(false);

    let output_path = args.output.as_ref().map(|p| p.as_path());
    let mut writer = create_writer(output_path, output_delimiter)?;

    let column_map = build_column_map(&headers);
    let sort_plan = build_sort_plan(&sorts, &schema, &column_map)?;
    let filter_conditions = filters;

    let output_plan = OutputPlan::new(
        &headers,
        &selected_columns,
        &derived_columns,
        args.row_numbers,
    )?;

    write_headers(&mut writer, &output_plan)?;

    if supports_index && !sorts.is_empty() {
        info!("Using index {:?} to accelerate sort", args.index);
        process_with_index(
            &mut reader,
            maybe_index.as_ref().unwrap(),
            &schema,
            &headers,
            &filter_conditions,
            &derived_columns,
            &output_plan,
            &mut writer,
            args.limit,
        )?
    } else {
        if maybe_index.is_some() {
            debug!("Index present but not used due to incompatible sort signature");
        }
        process_in_memory(
            reader,
            &schema,
            &headers,
            &filter_conditions,
            sort_plan,
            &derived_columns,
            &output_plan,
            &mut writer,
            args.limit,
        )?
    };
    writer.flush().context("Flushing output")
}

fn create_writer(
    path: Option<&Path>,
    delimiter: u8,
) -> Result<csv::Writer<Box<dyn std::io::Write>>> {
    let writer: Box<dyn std::io::Write> = match path {
        Some(path) => Box::new(BufWriter::new(
            File::create(path).with_context(|| format!("Creating output file {path:?}"))?,
        )),
        None => Box::new(std::io::stdout()),
    };
    Ok(WriterBuilder::new()
        .delimiter(delimiter)
        .from_writer(writer))
}

fn reconcile_schema_with_headers(schema: &mut Schema, headers: &[String]) {
    if schema.columns.len() == headers.len() {
        return;
    }
    schema.columns = headers
        .iter()
        .map(|name| crate::metadata::ColumnMeta {
            name: name.clone(),
            data_type: ColumnType::String,
        })
        .collect();
}

fn build_column_map(headers: &[String]) -> std::collections::HashMap<String, usize> {
    headers
        .iter()
        .enumerate()
        .map(|(idx, header)| (header.clone(), idx))
        .collect()
}

fn build_sort_plan(
    directives: &[SortDirective],
    schema: &Schema,
    column_map: &std::collections::HashMap<String, usize>,
) -> Result<Vec<SortInstruction>> {
    directives
        .iter()
        .map(|directive| {
            let idx = column_map
                .get(&directive.column)
                .copied()
                .or_else(|| schema.column_index(&directive.column))
                .ok_or_else(|| anyhow!("Sort column '{}' not found", directive.column))?;
            Ok(SortInstruction {
                index: idx,
                ascending: directive.ascending,
            })
        })
        .collect()
}

fn write_headers(
    writer: &mut csv::Writer<Box<dyn std::io::Write>>,
    plan: &OutputPlan,
) -> Result<()> {
    writer
        .write_record(plan.headers.iter())
        .context("Writing output headers")
}

fn process_in_memory(
    mut reader: csv::Reader<std::fs::File>,
    schema: &Schema,
    headers: &[String],
    filters: &[crate::filter::FilterCondition],
    sort_plan: Vec<SortInstruction>,
    derived_columns: &[DerivedColumn],
    output_plan: &OutputPlan,
    writer: &mut csv::Writer<Box<dyn std::io::Write>>,
    limit: Option<usize>,
) -> Result<()> {
    let mut record = ByteRecord::new();
    let mut rows: Vec<RowData> = Vec::new();
    let mut ordinal = 0usize;

    while reader.read_byte_record(&mut record)? {
        let raw = byte_record_to_strings(&record)?;
        let typed = parse_row(&raw, schema)?;

        if !filters.is_empty() && !evaluate_conditions(filters, schema, headers, &raw, &typed)? {
            continue;
        }

        rows.push(RowData {
            raw,
            typed,
            ordinal,
        });
        ordinal += 1;
    }

    if !sort_plan.is_empty() {
        rows.sort_by(|a, b| compare_rows(a, b, &sort_plan));
    }

    emit_rows(
        rows.into_iter(),
        headers,
        derived_columns,
        output_plan,
        writer,
        limit,
    )
}

fn process_with_index(
    reader: &mut csv::Reader<std::fs::File>,
    index: &CsvIndex,
    schema: &Schema,
    headers: &[String],
    filters: &[crate::filter::FilterCondition],
    derived_columns: &[DerivedColumn],
    output_plan: &OutputPlan,
    writer: &mut csv::Writer<Box<dyn std::io::Write>>,
    limit: Option<usize>,
) -> Result<()> {
    let mut record = ByteRecord::new();
    let mut emitted = 0usize;

    for offset in index.ordered_offsets() {
        if let Some(limit) = limit {
            if emitted >= limit {
                break;
            }
        }
        let mut position = Position::new();
        position.set_byte(offset);
        reader.seek(position)?;
        if !reader.read_byte_record(&mut record)? {
            break;
        }
        let raw = byte_record_to_strings(&record)?;
        let typed = parse_row(&raw, schema)?;
        if !filters.is_empty() && !evaluate_conditions(filters, schema, headers, &raw, &typed)? {
            continue;
        }
        emit_single_row(
            &raw,
            &typed,
            emitted + 1,
            headers,
            derived_columns,
            output_plan,
            writer,
        )?;
        emitted += 1;
    }
    Ok(())
}

fn emit_rows<I>(
    rows: I,
    headers: &[String],
    derived_columns: &[DerivedColumn],
    output_plan: &OutputPlan,
    writer: &mut csv::Writer<Box<dyn std::io::Write>>,
    limit: Option<usize>,
) -> Result<()>
where
    I: Iterator<Item = RowData>,
{
    let mut written = 0usize;
    for row in rows {
        if let Some(limit) = limit {
            if written >= limit {
                break;
            }
        }
        emit_single_row(
            &row.raw,
            &row.typed,
            written + 1,
            headers,
            derived_columns,
            output_plan,
            writer,
        )?;
        written += 1;
    }
    Ok(())
}

fn emit_single_row(
    raw: &[String],
    typed: &[Option<Value>],
    row_number: usize,
    headers: &[String],
    derived_columns: &[DerivedColumn],
    output_plan: &OutputPlan,
    writer: &mut csv::Writer<Box<dyn std::io::Write>>,
) -> Result<()> {
    let mut record = Vec::with_capacity(output_plan.fields.len());
    for field in &output_plan.fields {
        match field {
            OutputField::RowNumber => record.push(row_number.to_string()),
            OutputField::ExistingColumn(idx) => {
                let value = raw.get(*idx).cloned().unwrap_or_default();
                record.push(value);
            }
            OutputField::Derived(idx) => {
                let derived =
                    derived_columns[*idx].evaluate(headers, raw, typed, Some(row_number))?;
                record.push(derived);
            }
        }
    }
    writer
        .write_record(record.iter())
        .context("Writing output row")
}

fn parse_row(raw: &[String], schema: &Schema) -> Result<Vec<Option<Value>>> {
    schema
        .columns
        .iter()
        .enumerate()
        .map(|(idx, column)| {
            let value = raw.get(idx).map(|s| s.as_str()).unwrap_or("");
            parse_typed_value(value, &column.data_type)
        })
        .collect()
}

fn byte_record_to_strings(record: &ByteRecord) -> Result<Vec<String>> {
    record
        .iter()
        .map(|bytes| {
            std::str::from_utf8(bytes)
                .map(|s| s.to_string())
                .map_err(|err| anyhow!("Invalid UTF-8 in CSV record: {err}"))
        })
        .collect()
}

fn compare_rows(a: &RowData, b: &RowData, plan: &[SortInstruction]) -> std::cmp::Ordering {
    for directive in plan {
        let left = ComparableValue(a.typed[directive.index].clone());
        let right = ComparableValue(b.typed[directive.index].clone());
        let ord = left.cmp(&right);
        if ord != std::cmp::Ordering::Equal {
            return if directive.ascending {
                ord
            } else {
                ord.reverse()
            };
        }
    }
    a.ordinal.cmp(&b.ordinal)
}

#[derive(Debug)]
struct RowData {
    raw: Vec<String>,
    typed: Vec<Option<Value>>,
    ordinal: usize,
}

#[derive(Debug)]
struct SortInstruction {
    index: usize,
    ascending: bool,
}

#[derive(Debug)]
pub struct SortDirective {
    pub column: String,
    pub ascending: bool,
}

impl SortDirective {
    fn parse(spec: &str) -> Result<Self> {
        let mut parts = spec.split(':');
        let column = parts
            .next()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .ok_or_else(|| anyhow!("Sort directive is missing a column"))?;
        let direction = parts.next().unwrap_or("asc");
        let ascending = match direction.to_ascii_lowercase().as_str() {
            "asc" => true,
            "desc" => false,
            other => {
                return Err(anyhow!("Unknown sort direction '{other}'"));
            }
        };
        Ok(SortDirective {
            column: column.to_string(),
            ascending,
        })
    }
}

struct OutputPlan {
    headers: Vec<String>,
    fields: Vec<OutputField>,
}

impl OutputPlan {
    fn new(
        headers: &[String],
        selected_columns: &[String],
        derived: &[DerivedColumn],
        row_numbers: bool,
    ) -> Result<Self> {
        let mut fields = Vec::new();
        let mut output_headers = Vec::new();
        if row_numbers {
            fields.push(OutputField::RowNumber);
            output_headers.push("row_number".to_string());
        }
        let column_map = build_column_map(headers);
        let columns_to_use = if selected_columns.is_empty() {
            headers.to_vec()
        } else {
            selected_columns.to_vec()
        };
        for column in columns_to_use {
            let idx = column_map
                .get(&column)
                .copied()
                .ok_or_else(|| anyhow!("Requested column '{column}' not found"))?;
            fields.push(OutputField::ExistingColumn(idx));
            output_headers.push(headers[idx].clone());
        }
        for (idx, derived_column) in derived.iter().enumerate() {
            fields.push(OutputField::Derived(idx));
            output_headers.push(derived_column.name.clone());
        }
        Ok(OutputPlan {
            headers: output_headers,
            fields,
        })
    }
}

#[derive(Debug)]
enum OutputField {
    RowNumber,
    ExistingColumn(usize),
    Derived(usize),
}
