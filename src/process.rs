use std::fs::File;

use anyhow::{Context, Result, anyhow};
use csv::{ByteRecord, Position};
use itertools::Itertools;
use log::{debug, info};

use crate::{
    cli::{BooleanFormat, ProcessArgs},
    data::{ComparableValue, Value},
    derive::{DerivedColumn, parse_derived_columns},
    filter::{evaluate_conditions, parse_filters},
    index::{CsvIndex, IndexVariant, SortDirection},
    io_utils,
    rows::{evaluate_filter_expressions, parse_typed_row},
    schema::{ColumnMeta, ColumnType, Schema},
};

use encoding_rs::Encoding;

pub fn execute(args: &ProcessArgs) -> Result<()> {
    let delimiter = io_utils::resolve_input_delimiter(&args.input, args.delimiter);
    let input_encoding = io_utils::resolve_encoding(args.input_encoding.as_deref())?;
    let output_delimiter = io_utils::resolve_output_delimiter(
        args.output.as_deref(),
        args.output_delimiter,
        delimiter,
    );
    let output_encoding = io_utils::resolve_encoding(args.output_encoding.as_deref())?;
    info!(
        "Processing '{}' -> {:?} (delimiter '{}', output '{}')",
        args.input.display(),
        args.output
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "stdout".into()),
        crate::printable_delimiter(delimiter),
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

    let mut reader = io_utils::open_csv_reader_from_path(&args.input, delimiter, true)?;
    let headers = io_utils::reader_headers(&mut reader, input_encoding)?;

    let mut schema = if let Some(schema_path) = &args.schema {
        Schema::load(schema_path)?
    } else {
        Schema::from_headers(&headers)
    };

    reconcile_schema_with_headers(&mut schema, &headers)?;

    let maybe_index = if let Some(index_path) = &args.index {
        Some(CsvIndex::load(index_path)?)
    } else {
        None
    };

    let requested_variant = args
        .index_variant
        .as_ref()
        .map(|name| name.trim())
        .filter(|name| !name.is_empty())
        .map(|name| name.to_string());

    if requested_variant.is_some() && maybe_index.is_none() {
        return Err(anyhow!(
            "An index variant was specified but no index file was provided"
        ));
    }

    let sort_signature = sorts
        .iter()
        .map(|s| {
            (
                s.column.clone(),
                if s.ascending {
                    SortDirection::Asc
                } else {
                    SortDirection::Desc
                },
            )
        })
        .collect_vec();

    let matching_variant: Option<&IndexVariant> = if let Some(index) = maybe_index.as_ref() {
        if let Some(name) = requested_variant.as_deref() {
            if sort_signature.is_empty() {
                return Err(anyhow!(
                    "Selecting an index variant requires at least one --sort directive"
                ));
            }
            let variant = index.variant_by_name(name).ok_or_else(|| {
                anyhow!(
                    "Index variant '{name}' not found in {:?}",
                    args.index.as_ref().map(|p| p.display().to_string())
                )
            })?;
            if !variant.matches(&sort_signature) {
                return Err(anyhow!(
                    "Index variant '{name}' does not match the requested sort order"
                ));
            }
            Some(variant)
        } else if sort_signature.is_empty() {
            None
        } else {
            index.best_match(&sort_signature)
        }
    } else {
        None
    };

    let output_path = args.output.as_deref();
    let mut writer = io_utils::open_csv_writer(output_path, output_delimiter, output_encoding)?;

    let column_map = build_column_map(&headers, &schema);
    let sort_plan = build_sort_plan(&sorts, &schema, &column_map)?;
    let filter_conditions = filters;

    let output_plan = OutputPlan::new(
        &headers,
        &schema,
        &selected_columns,
        &derived_columns,
        args.row_numbers,
        args.boolean_format,
    )?;

    write_headers(&mut writer, &output_plan)?;
    {
        let mut engine = ProcessEngine {
            schema: &schema,
            headers: &headers,
            filters: &filter_conditions,
            filter_exprs: &args.filter_exprs,
            derived_columns: &derived_columns,
            output_plan: &output_plan,
            writer: &mut writer,
            limit: args.limit,
        };

        if let Some(variant) = matching_variant {
            if io_utils::is_dash(&args.input) {
                return Err(anyhow!(
                    "Index accelerated processing requires a regular file input"
                ));
            }
            let mut seek_reader = io_utils::open_seekable_csv_reader(&args.input, delimiter, true)?;
            // Read and discard headers to align reader position with data start.
            seek_reader.byte_headers()?;
            let covered = variant.columns().len();
            let total = sort_signature.len();
            info!(
                "Using index {:?} variant '{}' to accelerate sort",
                args.index,
                variant.describe()
            );
            if covered < total {
                debug!(
                    "Index covers {covered}/{total} sort columns; remaining columns will be sorted in-memory"
                );
            }
            engine.process_with_index(&mut seek_reader, input_encoding, variant, &sort_plan)?;
        } else {
            if maybe_index.is_some() {
                debug!("Index present but not used due to incompatible sort signature");
            }
            engine.process_in_memory(reader, input_encoding, sort_plan)?;
        }
    }
    writer.flush().context("Flushing output")
}

fn reconcile_schema_with_headers(schema: &mut Schema, headers: &[String]) -> Result<()> {
    if schema.columns.is_empty() {
        schema.columns = headers
            .iter()
            .map(|name| ColumnMeta {
                name: name.clone(),
                datatype: ColumnType::String,
                rename: None,
                value_replacements: Vec::new(),
            })
            .collect();
        return Ok(());
    }

    schema.validate_headers(headers)?;
    Ok(())
}

fn build_column_map(
    headers: &[String],
    schema: &Schema,
) -> std::collections::HashMap<String, usize> {
    let mut map = std::collections::HashMap::new();
    for (idx, header) in headers.iter().enumerate() {
        map.insert(header.clone(), idx);
        if let Some(rename) = schema
            .columns
            .get(idx)
            .and_then(|column| column.rename.as_ref())
            .filter(|rename| !rename.is_empty())
        {
            map.insert(rename.clone(), idx);
        }
    }
    map
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

struct ProcessEngine<'a> {
    schema: &'a Schema,
    headers: &'a [String],
    filters: &'a [crate::filter::FilterCondition],
    filter_exprs: &'a [String],
    derived_columns: &'a [DerivedColumn],
    output_plan: &'a OutputPlan,
    writer: &'a mut csv::Writer<Box<dyn std::io::Write>>,
    limit: Option<usize>,
}

impl<'a> ProcessEngine<'a> {
    fn process_in_memory(
        &mut self,
        reader: csv::Reader<Box<dyn std::io::Read>>,
        encoding: &'static Encoding,
        sort_plan: Vec<SortInstruction>,
    ) -> Result<()> {
        let mut rows: Vec<RowData> = Vec::new();

        for (ordinal, result) in reader.into_byte_records().enumerate() {
            let record = result.with_context(|| format!("Reading row {}", ordinal + 2))?;
            let mut raw = io_utils::decode_record(&record, encoding)?;
            self.schema.apply_replacements_to_row(&mut raw);
            let typed = parse_typed_row(self.schema, &raw)?;

            if !self.filters.is_empty()
                && !evaluate_conditions(self.filters, self.schema, self.headers, &raw, &typed)?
            {
                continue;
            }

            if !self.filter_exprs.is_empty()
                && !evaluate_filter_expressions(
                    self.filter_exprs,
                    self.headers,
                    &raw,
                    &typed,
                    Some(ordinal + 1),
                )?
            {
                continue;
            }

            rows.push(RowData {
                raw,
                typed,
                ordinal,
            });
        }

        if !sort_plan.is_empty() {
            rows.sort_by(|a, b| compare_rows(a, b, &sort_plan));
        }

        emit_rows(
            rows.into_iter(),
            self.headers,
            self.derived_columns,
            self.output_plan,
            self.writer,
            self.limit,
        )
    }

    fn process_with_index(
        &mut self,
        reader: &mut csv::Reader<std::io::BufReader<File>>,
        encoding: &'static Encoding,
        variant: &IndexVariant,
        sort_plan: &[SortInstruction],
    ) -> Result<()> {
        let mut record = ByteRecord::new();
        let mut emitted = 0usize;
        let mut ordinal = 0usize;
        let prefix_len = variant.columns().len();
        let mut current_prefix: Option<Vec<Option<Value>>> = None;
        let mut bucket: Vec<RowData> = Vec::new();

        for offset in variant.ordered_offsets() {
            if self.limit.is_some_and(|limit| emitted >= limit) {
                break;
            }
            let mut position = Position::new();
            position.set_byte(offset);
            reader.seek(position)?;
            if !reader.read_byte_record(&mut record)? {
                break;
            }
            let mut raw = io_utils::decode_record(&record, encoding)?;
            self.schema.apply_replacements_to_row(&mut raw);
            let typed = parse_typed_row(self.schema, &raw)?;
            if !self.filters.is_empty()
                && !evaluate_conditions(self.filters, self.schema, self.headers, &raw, &typed)?
            {
                continue;
            }

            if !self.filter_exprs.is_empty()
                && !evaluate_filter_expressions(
                    self.filter_exprs,
                    self.headers,
                    &raw,
                    &typed,
                    Some(ordinal + 1),
                )?
            {
                continue;
            }

            let prefix_key = build_prefix_key(&typed, sort_plan, prefix_len);
            match current_prefix.as_ref() {
                Some(existing) if *existing == prefix_key => {}
                Some(_) => {
                    if self.flush_bucket(&mut bucket, sort_plan, prefix_len, &mut emitted)? {
                        return Ok(());
                    }
                    current_prefix = Some(prefix_key.clone());
                }
                None => {
                    current_prefix = Some(prefix_key.clone());
                }
            }

            bucket.push(RowData {
                raw,
                typed,
                ordinal,
            });
            ordinal += 1;
        }

        self.flush_bucket(&mut bucket, sort_plan, prefix_len, &mut emitted)?;

        Ok(())
    }

    fn flush_bucket(
        &mut self,
        bucket: &mut Vec<RowData>,
        sort_plan: &[SortInstruction],
        prefix_len: usize,
        emitted: &mut usize,
    ) -> Result<bool> {
        if bucket.is_empty() {
            return Ok(false);
        }

        let remainder_plan = if prefix_len >= sort_plan.len() {
            &[][..]
        } else {
            &sort_plan[prefix_len..]
        };

        if !remainder_plan.is_empty() {
            bucket.sort_by(|a, b| compare_rows(a, b, remainder_plan));
        }

        for row in bucket.drain(..) {
            if self.limit.is_some_and(|limit| *emitted >= limit) {
                return Ok(true);
            }
            emit_single_row(
                &row.raw,
                &row.typed,
                *emitted + 1,
                self.headers,
                self.derived_columns,
                self.output_plan,
                self.writer,
            )?;
            *emitted += 1;
        }

        Ok(false)
    }
}

fn build_prefix_key(
    typed: &[Option<Value>],
    sort_plan: &[SortInstruction],
    prefix_len: usize,
) -> Vec<Option<Value>> {
    let take = prefix_len.min(sort_plan.len());
    sort_plan
        .iter()
        .take(take)
        .map(|directive| typed[directive.index].clone())
        .collect()
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
    for (written, row) in rows.enumerate() {
        if limit.is_some_and(|limit| written >= limit) {
            break;
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
                let raw_value = raw.get(*idx).map(String::as_str).unwrap_or("");
                let typed_value = typed.get(*idx).and_then(|v| v.as_ref());
                let formatted = output_plan.format_existing_value(raw_value, typed_value);
                record.push(formatted);
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
    boolean_format: BooleanFormat,
}

impl OutputPlan {
    fn new(
        headers: &[String],
        schema: &Schema,
        selected_columns: &[String],
        derived: &[DerivedColumn],
        row_numbers: bool,
        boolean_format: BooleanFormat,
    ) -> Result<Self> {
        let mut fields = Vec::new();
        let mut output_headers = Vec::new();
        if row_numbers {
            fields.push(OutputField::RowNumber);
            output_headers.push("row_number".to_string());
        }
        let column_map = build_column_map(headers, schema);
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
            output_headers.push(schema.columns[idx].output_name().to_string());
        }
        for (idx, derived_column) in derived.iter().enumerate() {
            fields.push(OutputField::Derived(idx));
            output_headers.push(derived_column.name.clone());
        }
        Ok(OutputPlan {
            headers: output_headers,
            fields,
            boolean_format,
        })
    }

    fn format_existing_value(&self, raw: &str, typed: Option<&Value>) -> String {
        match (self.boolean_format, typed) {
            (BooleanFormat::Original, _) => raw.to_string(),
            (BooleanFormat::TrueFalse, Some(Value::Boolean(true))) => "true".to_string(),
            (BooleanFormat::TrueFalse, Some(Value::Boolean(false))) => "false".to_string(),
            (BooleanFormat::OneZero, Some(Value::Boolean(true))) => "1".to_string(),
            (BooleanFormat::OneZero, Some(Value::Boolean(false))) => "0".to_string(),
            _ => raw.to_string(),
        }
    }
}

#[derive(Debug)]
enum OutputField {
    RowNumber,
    ExistingColumn(usize),
    Derived(usize),
}
