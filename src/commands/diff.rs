use crate::{cli::Invocation, error::PhError, io};
use polars::prelude::*;
use polars::series::ops::NullBehavior;

pub fn run(df: DataFrame, inv: &Invocation) -> Result<Vec<u8>, PhError> {
    let periods: i64 = inv
        .option("periods")
        .unwrap_or("1")
        .parse()
        .map_err(|_| PhError::new("diff --periods must be an integer."))?;
    let axis: u8 = inv
        .option("axis")
        .unwrap_or("0")
        .parse()
        .map_err(|_| PhError::new("diff --axis must be 0 or 1."))?;
    if axis > 1 {
        return Err(PhError::new("diff --axis must be 0 or 1."));
    }

    // Explicit column arguments select which columns are replaced by differences.
    // With no arguments, pandas diff() transforms every column.
    let names: Vec<String> = if inv.args.is_empty() {
        df.get_column_names()
            .iter()
            .map(|name| name.to_string())
            .collect()
    } else {
        inv.args.clone()
    };

    for name in &names {
        if df.column(name.as_str()).is_err() {
            return Err(PhError::new(format!("ph diff: Unknown column {name}")));
        }
    }

    let expressions: Vec<Expr> = if axis == 0 {
        names
            .iter()
            .map(|name| {
                // A pandas read_csv int64 column becomes float64 after diff,
                // because the leading difference is missing. Polars normally
                // retains an integer dtype, hence the explicit cast.
                col(name.as_str())
                    .cast(DataType::Float64)
                    .diff(lit(periods), NullBehavior::Ignore)
                    .alias(name.as_str())
            })
            .collect()
    } else {
        names
            .iter()
            .enumerate()
            .map(|(i, name)| {
                let j = i as i128 - periods as i128;
                if j < 0 || j >= names.len() as i128 {
                    // pandas uses float NaN for columns without a neighbor.
                    lit(NULL).cast(DataType::Float64).alias(name.as_str())
                } else {
                    let rhs = &names[j as usize];
                    let lhs_dtype = df.column(name.as_str()).unwrap().dtype();
                    let rhs_dtype = df.column(rhs.as_str()).unwrap().dtype();
                    // For axis=1, pandas preserves integral results when both
                    // columns are integral (unlike axis=0).
                    let dtype = if periods == 0
                        || matches!(lhs_dtype, DataType::Float32 | DataType::Float64)
                        || matches!(rhs_dtype, DataType::Float32 | DataType::Float64)
                    {
                        DataType::Float64
                    } else {
                        DataType::Int64
                    };
                    (col(name.as_str()).cast(dtype.clone()) - col(rhs.as_str()).cast(dtype))
                        .alias(name.as_str())
                }
            })
            .collect()
    };

    if expressions.is_empty() {
        return io::write_csv(df);
    }
    let output = df.lazy().with_columns(expressions).collect()?;
    io::write_csv(output)
}
