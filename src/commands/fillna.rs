//! Replace missing values using a scalar or forward/backward filling.
//! Matches the CLI of ph's `fillna`: either VALUE or --method, plus --limit.
use crate::{cli::Invocation, compat::missing, error::PhError, io};
use polars::prelude::*;

#[derive(Debug, Clone)]
enum Scalar {
    Integer(i64),
    Float(f64),
    Text(String),
}

impl Scalar {
    fn parse(value: &str) -> Result<Self, PhError> {
        if value == "None" {
            return Err(PhError::new(
                "Cannot fill with None; supply a value or --method.",
            ));
        }
        if let Ok(v) = value.parse::<i64>() {
            Ok(Self::Integer(v))
        } else if let Ok(v) = value.parse::<f64>() {
            Ok(Self::Float(v))
        } else {
            Ok(Self::Text(value.to_string()))
        }
    }
    fn number(&self) -> Option<f64> {
        match self {
            Self::Integer(v) => Some(*v as f64),
            Self::Float(v) if !v.is_nan() => Some(*v),
            Self::Float(_) => None,
            Self::Text(_) => None,
        }
    }
    fn as_text(&self) -> String {
        match self {
            Self::Integer(v) => v.to_string(),
            Self::Float(v) => format!("{v:?}"),
            Self::Text(v) => v.clone(),
        }
    }
}

#[derive(Clone, Copy)]
enum Method {
    Forward,
    Backward,
}

/// For scalar values, --limit is a total per-column quota. For pad/backfill,
/// it limits the length of *each* missing run (pandas semantics).
fn fill<T: Clone>(
    values: &mut [Option<T>],
    value: Option<T>,
    method: Option<Method>,
    limit: Option<usize>,
) {
    match method {
        None => {
            let mut filled = 0;
            for cell in values.iter_mut() {
                if cell.is_none()
                    && limit.is_none_or(|limit| filled < limit)
                    && let Some(value) = &value
                {
                    *cell = Some(value.clone());
                    filled += 1;
                }
            }
        }
        Some(method) => {
            let mut previous: Option<T> = None;
            let mut streak = 0_usize;
            let len = values.len();
            for position in 0..len {
                let index = match method {
                    Method::Forward => position,
                    Method::Backward => len - 1 - position,
                };
                let cell = &mut values[index];
                match cell {
                    Some(v) => {
                        previous = Some(v.clone());
                        streak = 0;
                    }
                    None => {
                        streak += 1;
                        if limit.is_none_or(|limit| streak <= limit) {
                            *cell = previous.clone();
                        }
                    }
                }
            }
        }
    }
}

/// Convert a nonmissing cell to the same textual representation that pandas
/// uses for CSV output, particularly for integral floating-point values.
fn cell_text(value: AnyValue<'_>) -> String {
    match value {
        // AnyValue::StringOwned implements Display with surrounding quotes.
        // Extract the actual string instead, letting CsvWriter perform the
        // necessary CSV quoting exactly once.
        AnyValue::String(v) => v.to_owned(),
        AnyValue::StringOwned(v) => v.as_str().to_owned(),
        AnyValue::Float64(v) => format!("{v:?}"),
        AnyValue::Float32(v) => format!("{v:?}"),
        AnyValue::Boolean(v) => if v { "True" } else { "False" }.to_owned(),
        other => other.to_string(),
    }
}

// Read string cells from the typed column, not AnyValue's formatting.
// Polars' display formatting can wrap string values in double quotes.
fn series_cell_text(series: &Series, row: usize) -> Option<String> {
    if series.dtype() == &DataType::String {
        return series.str().ok()?.get(row).map(str::to_owned);
    }
    series.get(row).ok().map(cell_text)
}

pub fn run(mut df: DataFrame, inv: &Invocation) -> Result<Vec<u8>, PhError> {
    let method = match inv.option("method") {
        None => None,
        Some("pad" | "ffill") => Some(Method::Forward),
        Some("backfill" | "bfill") => Some(Method::Backward),
        Some(other) => {
            return Err(PhError::new(format!(
                "fillna --method must be pad, ffill, backfill, or bfill, not {other}."
            )));
        }
    };
    let replacement = inv.args.first().map(|s| Scalar::parse(s)).transpose()?;
    if replacement.is_some() == method.is_some() {
        return Err(PhError::new(
            "fillna needs exactly one of VALUE and --method.",
        ));
    }
    let limit = inv
        .option("limit")
        .map(|s| {
            s.parse::<usize>().ok().filter(|&v| v > 0).ok_or_else(|| {
                PhError::new(format!(
                    "fillna --limit must be a positive integer, not {s}."
                ))
            })
        })
        .transpose()?;

    let names: Vec<String> = df
        .get_column_names()
        .iter()
        .map(ToString::to_string)
        .collect();
    for name in names {
        let column = df.column(&name)?;
        let series = column.as_materialized_series();
        let missing_mask = (0..df.height())
            .map(|row| series.get(row).map(missing::is_missing))
            .collect::<Result<Vec<_>, _>>()?;
        if !missing_mask.iter().any(|&missing| missing) {
            continue;
        }
        // Text columns containing numeric data and pandas NA tokens are
        // inferred as floats by pandas.read_csv (but not necessarily Polars).
        let string_column = matches!(column.dtype(), DataType::String | DataType::Null);
        let numeric_strings = string_column
            && (0..df.height()).all(|row| {
                missing_mask[row]
                    || series_cell_text(series, row).is_some_and(|v| v.parse::<f64>().is_ok())
            });
        let numeric_column = matches!(
            column.dtype(),
            DataType::Float32
                | DataType::Float64
                | DataType::Int8
                | DataType::Int16
                | DataType::Int32
                | DataType::Int64
                | DataType::UInt8
                | DataType::UInt16
                | DataType::UInt32
                | DataType::UInt64
        );

        if (numeric_column || numeric_strings)
            && (method.is_some() || replacement.as_ref().and_then(Scalar::number).is_some())
        {
            let mut values: Vec<Option<f64>> = (0..df.height())
                .map(|row| {
                    if missing_mask[row] {
                        None
                    } else {
                        series_cell_text(series, row).and_then(|v| v.parse::<f64>().ok())
                    }
                })
                .collect();
            fill(
                &mut values,
                replacement.as_ref().and_then(Scalar::number),
                method,
                limit,
            );
            df.replace(&name, Series::new(name.as_str().into(), values).into())?;
            continue;
        }

        // Strings and heterogeneous columns retain their nonmissing cells;
        // textual replacement of a numeric column promotes it to text.
        let mut values: Vec<Option<String>> = (0..df.height())
            .map(|row| {
                if missing_mask[row] {
                    None
                } else {
                    series_cell_text(series, row)
                }
            })
            .collect();
        fill(
            &mut values,
            replacement.as_ref().and_then(|v| {
                if matches!(v, Scalar::Float(n) if n.is_nan()) {
                    None
                } else {
                    Some(v.as_text())
                }
            }),
            method,
            limit,
        );
        df.replace(&name, Series::new(name.as_str().into(), values).into())?;
    }
    io::write_csv(df)
}
