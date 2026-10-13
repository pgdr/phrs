//! Round one numeric column. Integers never pass through f64.
use crate::{cli::Invocation, error::PhError, io};
use polars::prelude::*;

// Use i128 for exact rounding of all signed and unsigned 64-bit integers.
fn round_integer(value: i128, decimals: i32) -> i128 {
    if decimals >= 0 {
        return value;
    }
    let places = decimals.unsigned_abs();
    if places >= 20 {
        return 0; // Every i64/u64 value is smaller than half of 10^20.
    }
    let factor = 10_i128.pow(places);
    let quotient = value / factor;
    let remainder = (value % factor).abs();
    let ties_to_odd = 2 * remainder == factor && quotient % 2 != 0;
    let round_up = 2 * remainder > factor || ties_to_odd;
    let quotient = if round_up {
        quotient + value.signum()
    } else {
        quotient
    };
    quotient * factor
}

pub fn run(mut df: DataFrame, inv: &Invocation) -> Result<Vec<u8>, PhError> {
    let name = inv.args[0].as_str();
    if inv.args.len() > 1 && inv.option("decimals").is_some() {
        return Err(PhError::new("Specify decimals only once."));
    }
    let decimals: i32 = inv
        .option("decimals")
        .or_else(|| inv.args.get(1).map(String::as_str))
        .unwrap_or("0")
        .parse()
        .map_err(|_| PhError::new("round decimals must be an integer."))?;

    let column = df.column(name)?;
    let rounded = match column.dtype() {
        DataType::Int8 | DataType::Int16 | DataType::Int32 | DataType::Int64 => {
            if decimals >= 0 {
                return io::write_csv(df);
            }
            let col = column.cast(&DataType::Int64)?;
            let values = col.i64()?;
            let rounded: Vec<Option<i64>> = (0..values.len())
                .map(|i| {
                    values
                        .get(i)
                        .map(|v| {
                            i64::try_from(round_integer(i128::from(v), decimals))
                                .map_err(|_| PhError::new("Rounded integer out of range."))
                        })
                        .transpose()
                })
                .collect::<Result<_, _>>()?;
            Series::new(name.into(), rounded)
        }
        DataType::UInt8 | DataType::UInt16 | DataType::UInt32 | DataType::UInt64 => {
            if decimals >= 0 {
                return io::write_csv(df);
            }
            let col = column.cast(&DataType::UInt64)?;
            let values = col.u64()?;
            let rounded: Vec<Option<u64>> = (0..values.len())
                .map(|i| {
                    values
                        .get(i)
                        .map(|v| {
                            u64::try_from(round_integer(i128::from(v), decimals))
                                .map_err(|_| PhError::new("Rounded integer out of range."))
                        })
                        .transpose()
                })
                .collect::<Result<_, _>>()?;
            Series::new(name.into(), rounded)
        }
        DataType::Float32 | DataType::Float64 => {
            let col = column.cast(&DataType::Float64)?;
            let values = col.f64()?;
            let factor = 10_f64.powi(decimals);
            if !factor.is_finite() || factor == 0.0 {
                return Err(PhError::new("round decimals out of range."));
            }
            let rounded: Vec<Option<f64>> = (0..values.len())
                .map(|i| {
                    values.get(i).map(|v| {
                        let scaled = v * factor;
                        if scaled.is_finite() {
                            scaled.round_ties_even() / factor
                        } else {
                            v // Scaling overflow: fractional precision is already irrelevant.
                        }
                    })
                })
                .collect();
            Series::new(name.into(), rounded)
        }
        other => {
            return Err(PhError::new(format!(
                "Cannot round non-numeric column '{name}' (dtype {other})."
            )));
        }
    };
    df.replace(name, rounded.into())?;
    io::write_csv(df)
}

#[cfg(test)]
mod tests {
    use super::round_integer;

    #[test]
    fn integer_rounding_is_exact_and_ties_to_even() {
        assert_eq!(
            round_integer(9_007_199_254_740_993, 0),
            9_007_199_254_740_993
        );
        assert_eq!(
            round_integer(9_007_199_254_740_993, -1),
            9_007_199_254_740_990
        );
        assert_eq!(
            round_integer(9_007_199_254_740_995, -1),
            9_007_199_254_741_000
        );
        assert_eq!(round_integer(25, -1), 20);
        assert_eq!(round_integer(35, -1), 40);
        assert_eq!(round_integer(-25, -1), -20);
    }
}
