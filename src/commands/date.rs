//! Convert one CSV column to a date, following the common `ph date` cases.
use crate::{cli::Invocation, error::PhError, io};
use chrono::{DateTime, NaiveDate, Utc};
use polars::prelude::*;

fn bool_option(inv: &Invocation, key: &str) -> Result<bool, PhError> {
    if inv.flag(key) {
        return Ok(true);
    }
    match inv.option(key) {
        None | Some("False" | "false" | "0") => Ok(false),
        Some("True" | "true" | "1") => Ok(true),
        Some(other) => Err(PhError::new(format!("Invalid --{key} value: {other}."))),
    }
}

fn parse_date(value: &str, format: Option<&str>, dayfirst: bool) -> Result<NaiveDate, PhError> {
    let value = value.trim();

    // chrono needs a complete calendar date; pandas accepts year-only formats.
    if format == Some("%Y") {
        let value = format!("{value}-01-01");
        return NaiveDate::parse_from_str(&value, "%Y-%m-%d")
            .map_err(|_| PhError::new(format!("Invalid date: {value}.")));
    }

    let formats: &[&str] = if dayfirst {
        &["%Y-%m-%d", "%Y/%m/%d", "%d/%m/%Y", "%d-%m-%Y", "%m/%d/%Y"]
    } else {
        &["%Y-%m-%d", "%Y/%m/%d", "%m/%d/%Y", "%m-%d-%Y", "%d/%m/%Y"]
    };
    let parsed = if let Some(format) = format {
        NaiveDate::parse_from_str(value, format).ok()
    } else {
        formats
            .iter()
            .find_map(|format| NaiveDate::parse_from_str(value, format).ok())
    };
    parsed.ok_or_else(|| PhError::new(format!("Invalid date: {value}.")))
}

fn epoch_date(value: i64, utc: bool) -> Result<NaiveDate, PhError> {
    // The original ph interprets unformatted integers as *days*;
    // --utc=True interprets Unix timestamps in *seconds*.
    let seconds = if utc {
        value
    } else {
        value
            .checked_mul(86_400)
            .ok_or_else(|| PhError::new("Date out of range."))?
    };
    DateTime::<Utc>::from_timestamp(seconds, 0)
        .map(|timestamp| timestamp.date_naive())
        .ok_or_else(|| PhError::new("Date out of range."))
}

pub fn run(mut df: DataFrame, inv: &Invocation) -> Result<Vec<u8>, PhError> {
    let name = inv.args[0].as_str(); // validated to have exactly one argument
    let dayfirst = bool_option(inv, "dayfirst")?;
    let utc = bool_option(inv, "utc")?;
    let format = inv.option("format");

    let source = df.column(name)?;
    let numeric = matches!(
        source.dtype(),
        DataType::Int8
            | DataType::Int16
            | DataType::Int32
            | DataType::Int64
            | DataType::UInt8
            | DataType::UInt16
            | DataType::UInt32
            | DataType::UInt64
            | DataType::Float32
            | DataType::Float64
    );

    let dates: Vec<Option<NaiveDate>> = if numeric {
        if matches!(source.dtype(), DataType::Float32 | DataType::Float64) {
            // Nullable integer columns arrive as Float64 through io::read_csv.
            // Reject genuinely fractional values rather than silently truncate.
            let numbers = source.cast(&DataType::Float64)?;
            let floats = numbers.f64()?;
            for index in 0..floats.len() {
                let Some(number) = floats.get(index) else {
                    continue;
                };
                if !number.is_finite() || number.fract() != 0.0 {
                    return Err(PhError::new(format!(
                        "Fractional or non-finite date value: {number}."
                    )));
                }
            }
        }
        let numbers = source.cast(&DataType::Int64)?;
        let integers = numbers.i64()?;
        (0..integers.len())
            .map(|index| {
                integers
                    .get(index)
                    .map(|value| {
                        if let Some(format) = format {
                            parse_date(&value.to_string(), Some(format), dayfirst)
                        } else {
                            epoch_date(value, utc)
                        }
                    })
                    .transpose()
            })
            .collect::<Result<_, _>>()?
    } else if matches!(source.dtype(), DataType::String) {
        let strings = source.str()?;

        (0..strings.len())
            .map(|index| {
                strings
                    .get(index)
                    .map(|value| parse_date(value, format, dayfirst))
                    .transpose()
            })
            .collect::<Result<_, _>>()?
    } else {
        return Err(PhError::new(format!("Unsupported date dtype for {name}.")));
    };

    let parsed = DateChunked::from_naive_date_options(name.into(), dates).into_series();
    df.replace(name, parsed.into())?;
    io::write_csv(df)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn epoch_days() {
        assert_eq!(epoch_date(3, false).unwrap().to_string(), "1970-01-04");
    }

    #[test]
    fn unix_seconds() {
        assert_eq!(
            epoch_date(1_580_601_600, true).unwrap().to_string(),
            "2020-02-02"
        );
    }

    #[test]
    fn dayfirst_and_year_format() {
        assert_eq!(
            parse_date("01/04/2020", None, true).unwrap().to_string(),
            "2020-04-01"
        );
        assert_eq!(
            parse_date("2003", Some("%Y"), false).unwrap().to_string(),
            "2003-01-01"
        );
    }
}
