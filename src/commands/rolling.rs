//! Rolling numeric window reductions. Windows align to the right by default,
//! and match pandas' min_periods/window and sample-standard-deviation defaults.
use crate::{cli::Invocation, error::PhError, io};
use polars::prelude::*;
use super::statistics::{self, Statistic};

// Sliding sum and mean run in O(n), independent of window size.
fn sliding_sum_or_mean(
    data: &[Option<f64>],
    window: usize,
    center: bool,
    min_periods: usize,
    statistic: Statistic,
) -> Vec<Option<f64>> {
    let mut left = 0;
    let mut right = 0;
    let mut sum = 0.0;
    let mut count = 0_usize;
    let mut output = Vec::with_capacity(data.len());
    for row in 0..data.len() {
        let (before, after) = if center {
            (window / 2, (window - 1) / 2)
        } else {
            (window - 1, 0)
        };
        let start = row.saturating_sub(before);
        let end = row.saturating_add(after).saturating_add(1).min(data.len());
        while right < end {
            if let Some(value) = data[right] {
                sum += value;
                count += 1;
            }
            right += 1;
        }
        while left < start {
            if let Some(value) = data[left] {
                sum -= value;
                count -= 1;
            }
            left += 1;
        }
        output.push(if count < min_periods {
            None
        } else if statistic == Statistic::Sum {
            Some(sum)
        } else if count == 0 {
            None
        } else {
            Some(sum / count as f64)
        });
    }
    output
}

pub fn run(mut df: DataFrame, inv: &Invocation) -> Result<Vec<u8>, PhError> {
    let window = inv.args[0].parse::<usize>().map_err(|_| {
        PhError::new("rolling window must be a positive integer.")
    })?;
    if window == 0 || window > isize::MAX as usize {
        return Err(PhError::new("rolling window must be a positive integer."));
    }
    let statistic = Statistic::parse(inv.option("how").unwrap_or("sum"))?;
    let min_periods = inv.option("min_periods").unwrap_or("");
    let min_periods = if min_periods.is_empty() {
        window
    } else {
        min_periods.parse::<usize>().map_err(|_| {
            PhError::new("--min_periods must be a non-negative integer.")
        })?
    };
    if min_periods > window {
        return Err(PhError::new("--min_periods cannot exceed the window size."));
    }
    let center = statistics::boolean(inv.option("center").unwrap_or("False"), "center")?;
    let ddof = inv.option("ddof").unwrap_or("1").parse::<i64>().map_err(|_| {
        PhError::new("--ddof must be an integer.")
    })?;

    // With named columns, replace only those columns and leave the others
    // (e.g. date labels) unchanged. With no names, pandas' rolling reduction
    // operates on numeric columns and omits nonnumeric ones.
    let explicit = inv.args.len() > 1;
    let names: Vec<String> = if explicit {
        inv.args[1..].to_vec()
    } else {
        df.columns().iter()
            .filter(|col| statistics::is_numeric(col.dtype()))
            .map(|col| col.name().to_string())
            .collect()
    };
    if names.is_empty() {
        return Err(PhError::new("rolling needs at least one numeric column."));
    }
    for name in &names {
        let col = df.column(name.as_str()).map_err(|_| {
            PhError::new(format!("Unknown column {name}."))
        })?;
        if !statistics::is_numeric(col.dtype()) {
            return Err(PhError::new(format!("rolling requires a numeric column: {name}.")));
        }
        let converted = col.cast(&DataType::Float64)?;
        let data: Vec<Option<f64>> = converted.f64()?.iter().map(|item| {
            item.filter(|v| v.is_finite())
        }).collect();
        let result = if matches!(statistic, Statistic::Sum | Statistic::Mean) {
            sliding_sum_or_mean(&data, window, center, min_periods, statistic)
        } else {
            let mut result = Vec::with_capacity(data.len());
            for row in 0..data.len() {
                // For even centered windows, one more observation is on the left.
                let (before, after) = if center {
                    (window / 2, (window - 1) / 2)
                } else {
                    (window - 1, 0)
                };
                let start = row.saturating_sub(before);
                let end = row.saturating_add(after).saturating_add(1).min(data.len());
                let values: Vec<f64> = data[start..end].iter().filter_map(|&v| v).collect();
                result.push(if values.len() >= min_periods {
                    statistics::reduce_numeric(statistic, &values, ddof, 0)
                } else {
                    None
                });
            }
            result
        };
        df.replace(name, Series::new(name.as_str().into(), result).into())?;
    }
    if !explicit {
        return io::write_csv(df.select(names.iter().map(String::as_str))?);
    }
    io::write_csv(df)
}
