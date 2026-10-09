//! Column-wise reductions compatible with ph / pandas DataFrame reductions.
//! The input is a Polars DataFrame, but the single-row result is serialized
//! directly: this also accommodates pandas' mixed numeric/string sum/min/max.
use crate::{cli::Invocation, error::PhError};
use polars::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Statistic {
    Sum,
    Mean,
    Median,
    Std,
    Min,
    Max,
}

impl Statistic {
    pub fn parse(name: &str) -> Result<Self, PhError> {
        match name {
            "sum" => Ok(Self::Sum),
            "mean" => Ok(Self::Mean),
            "median" => Ok(Self::Median),
            "std" => Ok(Self::Std),
            "min" => Ok(Self::Min),
            "max" => Ok(Self::Max),
            _ => Err(PhError::new(format!("Unknown statistic: {name}."))),
        }
    }
}

pub fn is_numeric(dtype: &DataType) -> bool {
    matches!(
        dtype,
        DataType::Boolean
            | DataType::Int8
            | DataType::Int16
            | DataType::Int32
            | DataType::Int64
            | DataType::UInt8
            | DataType::UInt16
            | DataType::UInt32
            | DataType::UInt64
            | DataType::Float32
            | DataType::Float64
    )
}

fn is_integer(dtype: &DataType) -> bool {
    matches!(
        dtype,
        DataType::Int8
            | DataType::Int16
            | DataType::Int32
            | DataType::Int64
            | DataType::UInt8
            | DataType::UInt16
            | DataType::UInt32
            | DataType::UInt64
    )
}

pub fn boolean(value: &str, option: &str) -> Result<bool, PhError> {
    match value {
        "True" | "true" | "1" => Ok(true),
        "False" | "false" | "0" => Ok(false),
        _ => Err(PhError::new(format!("Invalid {option} value: {value}."))),
    }
}

fn float_csv(value: f64) -> String {
    // Debug's shortest round-trip representation retains the `.0` that
    // pandas prints for integral floating-point values.
    format!("{value:?}")
}

fn csv_one_row(headers: &[String], values: &[String]) -> Result<Vec<u8>, PhError> {
    let mut writer = csv::WriterBuilder::new().from_writer(Vec::new());
    writer
        .write_record(headers)
        .map_err(|e| PhError::new(e.to_string()))?;
    writer
        .write_record(values)
        .map_err(|e| PhError::new(e.to_string()))?;
    writer
        .into_inner()
        .map_err(|e| PhError::new(e.to_string()))
}

/// Compute sample standard deviation (pandas' default ddof is 1).
fn standard_deviation(data: &[f64], ddof: i64) -> Option<f64> {
    if (data.len() as i128) <= i128::from(ddof) || data.is_empty() {
        return None;
    }
    let mut mean = 0.0;
    let mut m2 = 0.0;
    for (index, &value) in data.iter().enumerate() {
        let n = (index + 1) as f64;
        let delta = value - mean;
        mean += delta / n;
        m2 += delta * (value - mean);
    }
    Some((m2 / (data.len() as f64 - ddof as f64)).sqrt())
}

pub fn reduce_numeric(
    statistic: Statistic,
    values: &[f64],
    ddof: i64,
    min_count: usize,
) -> Option<f64> {
    match statistic {
        Statistic::Sum if values.len() >= min_count => Some(values.iter().sum()),
        Statistic::Sum => None,
        Statistic::Mean if !values.is_empty() => {
            Some(values.iter().sum::<f64>() / values.len() as f64)
        }
        Statistic::Median if !values.is_empty() => {
            let mut sorted = values.to_vec();
            sorted.sort_by(f64::total_cmp);
            let mid = sorted.len() / 2;
            if sorted.len() % 2 == 0 {
                Some(sorted[mid - 1] / 2.0 + sorted[mid] / 2.0)
            } else {
                Some(sorted[mid])
            }
        }
        Statistic::Std => standard_deviation(values, ddof),
        Statistic::Min => values.iter().copied().reduce(f64::min),
        Statistic::Max => values.iter().copied().reduce(f64::max),
        _ => None,
    }
}

fn column_value(
    column: &Column,
    statistic: Statistic,
    skipna: bool,
    ddof: i64,
    min_count: usize,
) -> Result<Option<String>, PhError> {
    let dtype = column.dtype();
    if matches!(dtype, DataType::String) {
        if !matches!(statistic, Statistic::Sum | Statistic::Min | Statistic::Max) {
            return Err(PhError::new(format!(
                "Cannot apply {statistic:?} to non-numeric column '{}'.",
                column.name()
            )));
        }
        let mut strings = Vec::new();
        for item in column.str()?.iter() {
            match item {
                Some(s) => strings.push(s),
                None if !skipna => return Ok(None),
                _ => {}
            }
        }
        return Ok(match statistic {
            Statistic::Sum if strings.len() >= min_count => Some(strings.concat()),
            Statistic::Sum => None,
            Statistic::Min => strings.into_iter().min().map(|v| v.to_owned()),
            Statistic::Max => strings.into_iter().max().map(|v| v.to_owned()),
            _ => unreachable!(),
        });
    }
    if !is_numeric(dtype) {
        return Err(PhError::new(format!(
            "Unsupported dtype for column '{}': {dtype}.",
            column.name()
        )));
    }
    if is_integer(dtype) && matches!(statistic, Statistic::Sum | Statistic::Min | Statistic::Max) {
        let mut ints = Vec::new();
        for index in 0..column.len() {
            let value = column.as_materialized_series().get(index)?;
            if matches!(&value, AnyValue::Null) {
                if !skipna {
                    return Ok(None);
                }
                continue;
            }
            ints.push(value.to_string().parse::<i128>().map_err(|_| {
                PhError::new(format!("Invalid integer in '{}'.", column.name()))
            })?);
        }
        return Ok(match statistic {
            Statistic::Sum if ints.len() < min_count => None,
            Statistic::Sum => {
                let sum = ints.iter().try_fold(0_i128, |acc, &n| acc.checked_add(n))
                    .ok_or_else(|| PhError::new("Integer overflow in sum."))?;
                Some(sum.to_string())
            }
            Statistic::Min => ints.into_iter().min().map(|v| v.to_string()),
            Statistic::Max => ints.into_iter().max().map(|v| v.to_string()),
            _ => unreachable!(),
        });
    }
    let converted = column.cast(&DataType::Float64)?;
    let mut nums = Vec::with_capacity(column.len());
    for item in converted.f64()?.iter() {
        match item {
            Some(v) if !v.is_nan() => nums.push(v),
            _ if !skipna => return Ok(None),
            _ => {}
        }
    }
    // A Boolean sum is an integer count, whereas means and medians are float.
    if matches!(dtype, DataType::Boolean) && matches!(statistic, Statistic::Sum) {
        return Ok((nums.len() >= min_count).then(|| {
            nums.iter().filter(|&&v| v != 0.0).count().to_string()
        }));
    }
    if matches!(dtype, DataType::Boolean) && matches!(statistic, Statistic::Min | Statistic::Max) {
        return Ok(reduce_numeric(statistic, &nums, ddof, min_count)
            .map(|v| if v != 0.0 { "True" } else { "False" }.to_owned()));
    }
    Ok(reduce_numeric(statistic, &nums, ddof, min_count).map(float_csv))
}

pub fn run(df: DataFrame, inv: &Invocation) -> Result<Vec<u8>, PhError> {
    let statistic = Statistic::parse(&inv.command)?;
    let axis = match inv.option("axis").unwrap_or("0") {
        "0" | "index" => 0,
        "1" | "columns" => 1,
        value => return Err(PhError::new(format!("Invalid axis: {value}."))),
    };
    let skipna = boolean(inv.option("skipna").unwrap_or("True"), "skipna")?;
    let numeric_only = boolean(inv.option("numeric_only").unwrap_or("False"), "numeric_only")?;
    let ddof = inv.option("ddof").unwrap_or("1").parse::<i64>()
        .map_err(|_| PhError::new("--ddof must be an integer."))?;
    let min_count = inv.option("min_count").unwrap_or("0").parse::<usize>()
        .map_err(|_| PhError::new("--min_count must be a non-negative integer."))?;

    let names: Vec<String> = if inv.args.is_empty() {
        df.get_column_names().iter().map(|n| n.to_string()).collect()
    } else {
        inv.args.clone()
    };
    let mut selected = Vec::new();
    for name in &names {
        let col = df.column(name.as_str()).map_err(|_| PhError::new(format!("Unknown column {name}.")))?;
        if !numeric_only || is_numeric(col.dtype()) {
            selected.push(col);
        }
    }
    if selected.is_empty() {
        return Err(PhError::new("No columns selected for aggregation."));
    }
    if axis == 0 {
        let mut headers = Vec::new();
        let mut results = Vec::new();
        let mut float_present = false;
        let mut string_present = false;
        for col in &selected {
            let dtype = col.dtype();
            float_present |= matches!(dtype, DataType::Float32 | DataType::Float64);
            string_present |= matches!(dtype, DataType::String);
            headers.push(col.name().to_string());
            results.push(column_value(col, statistic, skipna, ddof, min_count)?);
        }
        // pandas forms one homogeneous numeric Series before transposing it.
        // Mixed integer and floating columns therefore have floating results.
        if float_present && !string_present && matches!(statistic, Statistic::Sum | Statistic::Min | Statistic::Max) {
            for value in &mut results {
                if let Some(v) = value {
                    if !v.chars().any(|c| matches!(c, '.' | 'e' | 'E')) && v != "True" && v != "False" {
                        *v = float_csv(v.parse::<f64>().map_err(|_| PhError::new("Numeric conversion failed."))?);
                    }
                }
            }
        }
        return csv_one_row(&headers, &results.into_iter().map(Option::unwrap_or_default).collect::<Vec<_>>());
    }
    // pandas axis=1 returns a Series indexed by the original row numbers;
    // ph transposes that to a one-row CSV with 0,1,... headers.
    let integer_output = matches!(statistic, Statistic::Sum | Statistic::Min | Statistic::Max)
        && selected.iter().all(|c| is_integer(c.dtype()) || matches!(c.dtype(), DataType::Boolean));
    let mut columns = Vec::new();
    for col in selected {
        if !is_numeric(col.dtype()) {
            return Err(PhError::new(format!("Cannot aggregate non-numeric column '{}' across rows.", col.name())));
        }
        columns.push(col.cast(&DataType::Float64)?);
    }
    let mut values = Vec::with_capacity(df.height());
    for row in 0..df.height() {
        let mut nums = Vec::with_capacity(columns.len());
        let mut missing = false;
        for col in &columns {
            match col.f64()?.get(row) {
                Some(v) if !v.is_nan() => nums.push(v),
                _ => missing = true,
            }
        }
        let result = if missing && !skipna { None } else { reduce_numeric(statistic, &nums, ddof, min_count) };
        values.push(match result {
            Some(v) if integer_output => format!("{v:.0}"),
            Some(v) => float_csv(v),
            None => String::new(),
        });
    }
    let headers = (0..df.height()).map(|i| i.to_string()).collect::<Vec<_>>();
    csv_one_row(&headers, &values)
}
