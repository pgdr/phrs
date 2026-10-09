//! Remove rows or columns using pandas/ph's dropna rules.
use crate::{cli::Invocation, compat::missing, error::PhError, io};
use polars::prelude::*;

pub fn run(df: DataFrame, inv: &Invocation) -> Result<Vec<u8>, PhError> {
    let axis = match inv.option("axis").unwrap_or("0") {
        "0" => 0,
        "1" => 1,
        value => {
            return Err(PhError::new(format!(
                "dropna --axis must be 0 or 1, not {value}."
            )));
        }
    };
    let threshold = inv
        .option("thresh")
        .map(|s| {
            s.parse::<i64>()
                .map_err(|_| PhError::new(format!("dropna --thresh must be an integer, not {s}.")))
        })
        .transpose()?;
    let how = inv.option("how").unwrap_or("any");
    if threshold.is_none() && how != "any" && how != "all" {
        return Err(PhError::new(format!(
            "dropna --how must be any or all, not {how}."
        )));
    }

    let rows = df.height();
    let cols = df.width();
    let mut valid_in_row = vec![0_usize; rows];
    let mut valid_in_column = vec![0_usize; cols];
    for (column_index, column) in df.columns().iter().enumerate() {
        let series = column.as_materialized_series();
        for (row, count) in valid_in_row.iter_mut().enumerate() {
            if !missing::is_missing(series.get(row)?) {
                *count += 1;
                valid_in_column[column_index] += 1;
            }
        }
    }

    // --thresh takes precedence over --how, exactly as in the original ph.
    let retain = |present: usize, total: usize| -> bool {
        match threshold {
            Some(required) => (present as i128) >= i128::from(required),
            None if how == "all" => present > 0,
            None => present == total,
        }
    };
    if axis == 0 {
        let keep: Vec<bool> = valid_in_row.into_iter().map(|n| retain(n, cols)).collect();
        let mask = BooleanChunked::from_slice("keep".into(), &keep);
        io::write_csv(df.filter(&mask)?)
    } else {
        let names: Vec<String> = df
            .get_column_names()
            .iter()
            .zip(valid_in_column)
            .filter(|&(_name, n)| retain(n, rows))
            .map(|(name, _n)| name.to_string())
            .collect();
        if names.is_empty() {
            // pandas writes a blank header and a blank line per input row
            // when every column is dropped.
            return Ok(vec![b'\n'; rows + 1]);
        }
        io::write_csv(df.select(names.iter().map(String::as_str))?)
    }
}
