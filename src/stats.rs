use std::collections::HashMap;

use anyhow::{Context, Result, anyhow, bail};
use chrono::{DateTime, Datelike, NaiveDate, NaiveDateTime, NaiveTime, Timelike, Utc};
use encoding_rs::Encoding;
use log::info;

use crate::{
    cli::StatsArgs,
    data::Value,
    filter, frequency, io_utils,
    rows::{evaluate_filter_expressions, parse_typed_row},
    schema::{self, ColumnType, DecimalSpec, Schema},
    table,
};

pub fn execute(args: &StatsArgs) -> Result<()> {
    if args.schema.is_none() && io_utils::is_dash(&args.input) {
        return Err(anyhow!(
            "Reading from stdin requires --schema (or --meta) for stats operations"
        ));
    }

    let delimiter = io_utils::resolve_input_delimiter(&args.input, args.delimiter);
    let encoding = io_utils::resolve_encoding(args.input_encoding.as_deref())?;

    let schema = load_or_infer_schema(args, delimiter, encoding)?;

    let columns = resolve_columns(&schema, &args.columns, args.frequency)?;
    if columns.is_empty() {
        if args.frequency {
            return Err(anyhow!(
                "No columns available for frequency analysis. Supply --columns to continue."
            ));
        }
        return Err(anyhow!(
            "No numeric or temporal columns available. Provide a schema file or explicit column list."
        ));
    }

    let filters = filter::parse_filters(&args.filters)?;

    if args.frequency {
        let freq_options = frequency::FrequencyOptions {
            top: args.top,
            row_limit: (args.limit > 0).then_some(args.limit),
            filters: &filters,
            filter_exprs: &args.filter_exprs,
        };
        let rows = frequency::compute_frequency_rows(
            &args.input,
            &schema,
            delimiter,
            encoding,
            &columns,
            &freq_options,
        )?;
        let headers = vec![
            "column".to_string(),
            "value".to_string(),
            "count".to_string(),
            "percent".to_string(),
        ];
        table::print_table(&headers, &rows);
        info!("Computed frequency counts for {} column(s)", columns.len());
        return Ok(());
    }

    let expects_headers = schema.expects_headers();
    let mut reader = io_utils::open_csv_reader_from_path(&args.input, delimiter, expects_headers)?;
    let headers = if expects_headers {
        let headers = io_utils::reader_headers(&mut reader, encoding)?;
        schema
            .validate_headers(&headers)
            .with_context(|| format!("Validating headers for {:?}", args.input))?;
        headers
    } else {
        schema.headers()
    };
    let header_aliases = schema.header_alias_sets();

    let mut stats = StatsAccumulator::new(&columns, &schema);

    for (row_idx, record) in reader.byte_records().enumerate() {
        if args.limit > 0 && row_idx >= args.limit {
            break;
        }
        let record = record.with_context(|| format!("Reading row {}", row_idx + 2))?;
        let mut decoded = io_utils::decode_record(&record, encoding)?;
        if schema::row_looks_like_header(&decoded, &header_aliases) {
            continue;
        }
        if schema.has_transformations() {
            schema
                .apply_transformations_to_row(&mut decoded)
                .with_context(|| {
                    format!(
                        "Applying datatype mappings to row {} in {:?}",
                        row_idx + 2,
                        args.input
                    )
                })?;
        }
        schema.apply_replacements_to_row(&mut decoded);
        let typed = parse_typed_row(&schema, &decoded)
            .with_context(|| format!("Parsing row {}", row_idx + 2))?;
        if !filters.is_empty()
            && !filter::evaluate_conditions(&filters, &schema, &headers, &decoded, &typed)?
        {
            continue;
        }
        if !args.filter_exprs.is_empty()
            && !evaluate_filter_expressions(
                &args.filter_exprs,
                &headers,
                &decoded,
                &typed,
                Some(row_idx + 1),
            )?
        {
            continue;
        }
        stats
            .ingest(&typed)
            .with_context(|| format!("Processing row {}", row_idx + 2))?;
    }

    let rows = stats.render_rows();
    let headers = vec![
        "column".to_string(),
        "count".to_string(),
        "min".to_string(),
        "max".to_string(),
        "mean".to_string(),
        "median".to_string(),
        "std_dev".to_string(),
    ];
    table::print_table(&headers, &rows);
    info!("Computed summary statistics for {} column(s)", rows.len());
    Ok(())
}

fn load_or_infer_schema(
    args: &StatsArgs,
    delimiter: u8,
    encoding: &'static Encoding,
) -> Result<Schema> {
    if let Some(path) = &args.schema {
        Schema::load(path).with_context(|| format!("Loading schema from {path:?}"))
    } else {
        schema::infer_schema(&args.input, 0, delimiter, encoding, None)
            .with_context(|| format!("Inferring schema from {input:?}", input = args.input))
    }
}

fn resolve_columns(
    schema: &Schema,
    specified: &[String],
    frequency_mode: bool,
) -> Result<Vec<usize>> {
    if frequency_mode {
        if specified.is_empty() {
            Ok((0..schema.columns.len()).collect())
        } else {
            specified
                .iter()
                .map(|name| {
                    schema
                        .column_index(name)
                        .ok_or_else(|| anyhow!("Column '{name}' not found in schema"))
                })
                .collect()
        }
    } else if specified.is_empty() {
        Ok(schema
            .columns
            .iter()
            .enumerate()
            .filter(|(_, col)| is_supported_datatype(&col.datatype))
            .map(|(idx, _)| idx)
            .collect())
    } else {
        specified
            .iter()
            .map(|name| {
                let idx = schema
                    .column_index(name)
                    .ok_or_else(|| anyhow!("Column '{name}' not found in schema"))?;
                let column = &schema.columns[idx];
                if !is_supported_datatype(&column.datatype) {
                    return Err(anyhow!(
                        "Column '{}' is type {:?} and cannot be profiled for statistics",
                        column.output_name(),
                        column.datatype
                    ));
                }
                Ok(idx)
            })
            .collect()
    }
}

fn is_supported_datatype(datatype: &ColumnType) -> bool {
    matches!(
        datatype,
        ColumnType::Integer
            | ColumnType::Float
            | ColumnType::Currency
            | ColumnType::Decimal(_)
            | ColumnType::Date
            | ColumnType::DateTime
            | ColumnType::Time
    )
}

struct StatsAccumulator {
    columns: Vec<usize>,
    data: HashMap<usize, ColumnStats>,
}

impl StatsAccumulator {
    fn new(columns: &[usize], schema: &Schema) -> Self {
        let mut data = HashMap::new();
        for idx in columns {
            let stats = ColumnStats::with_column(
                schema.columns[*idx].output_name().to_string(),
                schema.columns[*idx].datatype.clone(),
            );
            data.insert(*idx, stats);
        }
        Self {
            columns: columns.to_vec(),
            data,
        }
    }

    fn ingest(&mut self, typed_row: &[Option<Value>]) -> Result<()> {
        for column_index in &self.columns {
            if let Some(stats) = self.data.get_mut(column_index)
                && let Some(Some(value)) = typed_row.get(*column_index)
            {
                let column_name = stats.name.clone();
                stats
                    .add_value(value)
                    .with_context(|| format!("Column '{}'", column_name))?;
            }
        }
        Ok(())
    }

    fn render_rows(&self) -> Vec<Vec<String>> {
        let mut rows = Vec::new();
        for column_index in &self.columns {
            if let Some(stats) = self.data.get(column_index) {
                rows.push(stats.render_row());
            }
        }
        rows
    }
}

struct ColumnStats {
    name: String,
    datatype: ColumnType,
    values: Vec<f64>,
    sum: f64,
    sum_squares: f64,
    count: usize,
    min: Option<f64>,
    max: Option<f64>,
    currency_scale: Option<u32>,
    decimal_scale: Option<u32>,
}

impl ColumnStats {
    fn with_column(name: String, datatype: ColumnType) -> Self {
        Self {
            name,
            datatype,
            values: Vec::new(),
            sum: 0.0,
            sum_squares: 0.0,
            count: 0,
            min: None,
            max: None,
            currency_scale: None,
            decimal_scale: None,
        }
    }

    fn add_value(&mut self, value: &Value) -> Result<()> {
        if let (ColumnType::Currency, Value::Currency(currency)) = (&self.datatype, value) {
            let scale = currency.scale();
            self.currency_scale = Some(
                self.currency_scale
                    .map_or(scale, |current| current.max(scale)),
            );
        }
        if let (ColumnType::Decimal(_), Value::Decimal(decimal)) = (&self.datatype, value) {
            let scale = decimal.scale();
            self.decimal_scale = Some(
                self.decimal_scale
                    .map_or(scale, |current| current.max(scale)),
            );
        }
        let numeric = value_to_metric(value, &self.datatype)?;
        self.count += 1;
        self.sum += numeric;
        self.sum_squares += numeric * numeric;
        self.min = Some(match self.min {
            Some(current) => current.min(numeric),
            None => numeric,
        });
        self.max = Some(match self.max {
            Some(current) => current.max(numeric),
            None => numeric,
        });
        self.values.push(numeric);
        Ok(())
    }

    fn mean(&self) -> Option<f64> {
        if self.count > 0 {
            Some(self.sum / self.count as f64)
        } else {
            None
        }
    }

    fn median(&self) -> Option<f64> {
        if self.values.is_empty() {
            return None;
        }
        let mut sorted = self.values.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let mid = sorted.len() / 2;
        if sorted.len().is_multiple_of(2) {
            Some((sorted[mid - 1] + sorted[mid]) / 2.0)
        } else {
            Some(sorted[mid])
        }
    }

    fn std_dev(&self) -> Option<f64> {
        if self.count < 2 {
            return None;
        }
        let mean = self.mean()?;
        let variance =
            (self.sum_squares - self.count as f64 * mean * mean) / (self.count as f64 - 1.0);
        Some(variance.max(0.0).sqrt())
    }

    fn render_row(&self) -> Vec<String> {
        vec![
            self.name.clone(),
            self.count.to_string(),
            self.format_metric(self.min),
            self.format_metric(self.max),
            self.format_metric(self.mean()),
            self.format_metric(self.median()),
            self.format_std_dev(self.std_dev()),
        ]
    }

    fn format_metric(&self, metric: Option<f64>) -> String {
        metric
            .map(|value| {
                format_metric(
                    value,
                    &self.datatype,
                    self.currency_scale,
                    self.decimal_scale,
                )
            })
            .unwrap_or_default()
    }

    fn format_std_dev(&self, metric: Option<f64>) -> String {
        metric
            .map(|value| {
                format_std_dev_value(
                    value,
                    &self.datatype,
                    self.currency_scale,
                    self.decimal_scale,
                )
            })
            .unwrap_or_default()
    }
}

fn format_number(value: f64) -> String {
    if value.fract() == 0.0 {
        format!("{value:.0}")
    } else {
        format!("{value:.4}")
    }
}

fn value_to_metric(value: &Value, datatype: &ColumnType) -> Result<f64> {
    match (datatype, value) {
        (ColumnType::Integer, Value::Integer(i)) => Ok(*i as f64),
        (ColumnType::Float, Value::Float(f)) => Ok(*f),
        (ColumnType::Float, Value::Integer(i)) => Ok(*i as f64),
        (ColumnType::Currency, Value::Currency(c)) => c
            .to_f64()
            .ok_or_else(|| anyhow!("Currency value out of range for statistics")),
        (ColumnType::Decimal(_), Value::Decimal(d)) => d
            .to_f64()
            .ok_or_else(|| anyhow!("Decimal value out of range for statistics")),
        (ColumnType::Date, Value::Date(d)) => Ok(date_to_metric(d)),
        (ColumnType::DateTime, Value::DateTime(dt)) => Ok(datetime_to_metric(dt)),
        (ColumnType::Time, Value::Time(t)) => Ok(time_to_metric(t)),
        _ => bail!("Value {:?} incompatible with datatype {datatype:?}", value),
    }
}

fn date_to_metric(date: &NaiveDate) -> f64 {
    date.num_days_from_ce() as f64
}

fn datetime_to_metric(dt: &NaiveDateTime) -> f64 {
    dt.and_utc().timestamp() as f64
}

fn time_to_metric(time: &NaiveTime) -> f64 {
    time.num_seconds_from_midnight() as f64
}

fn metric_to_date(metric: f64) -> Option<NaiveDate> {
    NaiveDate::from_num_days_from_ce_opt(metric.round() as i32)
}

fn metric_to_datetime(metric: f64) -> Option<NaiveDateTime> {
    if metric.is_nan() || metric.is_infinite() {
        return None;
    }
    DateTime::<Utc>::from_timestamp(metric.round() as i64, 0).map(|dt| dt.naive_utc())
}

fn metric_to_time(metric: f64) -> Option<NaiveTime> {
    let mut seconds = metric.round();
    if seconds.is_nan() || seconds.is_infinite() {
        return None;
    }
    if seconds < 0.0 {
        seconds = 0.0;
    }
    if seconds >= 86_400.0 {
        seconds = 86_399.0;
    }
    NaiveTime::from_num_seconds_from_midnight_opt(seconds as u32, 0)
}

fn format_metric(
    value: f64,
    datatype: &ColumnType,
    currency_scale: Option<u32>,
    decimal_scale: Option<u32>,
) -> String {
    match datatype {
        ColumnType::Integer | ColumnType::Float => format_number(value),
        ColumnType::Currency => format_currency_number(value, currency_scale),
        ColumnType::Decimal(spec) => format_decimal_number(value, spec, decimal_scale),
        ColumnType::Date => metric_to_date(value)
            .map(|d| d.format("%Y-%m-%d").to_string())
            .unwrap_or_default(),
        ColumnType::DateTime => metric_to_datetime(value)
            .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
            .unwrap_or_default(),
        ColumnType::Time => metric_to_time(value)
            .map(|t| t.format("%H:%M:%S").to_string())
            .unwrap_or_default(),
        _ => String::new(),
    }
}

fn format_std_dev_value(
    value: f64,
    datatype: &ColumnType,
    currency_scale: Option<u32>,
    decimal_scale: Option<u32>,
) -> String {
    match datatype {
        ColumnType::Integer | ColumnType::Float => format_number(value),
        ColumnType::Currency => format_currency_number(value, currency_scale),
        ColumnType::Decimal(spec) => format_decimal_number(value, spec, decimal_scale),
        ColumnType::Date => format_duration(value, "days"),
        ColumnType::DateTime | ColumnType::Time => format_duration(value, "seconds"),
        _ => String::new(),
    }
}

fn format_currency_number(value: f64, scale: Option<u32>) -> String {
    if value.is_nan() || value.is_infinite() {
        return String::new();
    }
    let digits = match scale.unwrap_or(2) {
        4 => 4,
        _ => 2,
    };
    format!("{value:.precision$}", precision = digits as usize)
}

fn format_decimal_number(value: f64, spec: &DecimalSpec, observed_scale: Option<u32>) -> String {
    if value.is_nan() || value.is_infinite() {
        return String::new();
    }
    let digits = observed_scale.unwrap_or(spec.scale) as usize;
    if digits == 0 {
        format!("{value:.0}")
    } else {
        format!("{value:.precision$}", precision = digits)
    }
}

fn format_duration(value: f64, unit: &str) -> String {
    let magnitude = format_number(value.abs());
    if magnitude.is_empty() {
        return String::new();
    }
    if value < 0.0 {
        format!("-{magnitude} {unit}")
    } else {
        format!("{magnitude} {unit}")
    }
}

