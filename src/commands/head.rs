use crate::{cli::Invocation, commands::count_arg, error::PhError, io};
use polars::prelude::*;

/// pandas DataFrame.head(n): negative n excludes the last |n| rows.
pub fn run(df: DataFrame, inv: &Invocation) -> Result<Vec<u8>, PhError> {
    let n = count_arg(inv, 10)?;
    let len = df.height();
    let selected = if n >= 0 {
        df.head(Some((n as u64).min(len as u64) as usize))
    } else {
        df.head(Some(len.saturating_sub(n.unsigned_abs() as usize)))
    };
    io::write_csv(selected)
}
