use crate::error::PhError;
use polars::prelude::*;
/// Provisional shape formatting; validate against the pinned Python oracle.
pub fn run(df: DataFrame) -> Result<Vec<u8>, PhError> {
    Ok(format!("rows,columns\n{},{}\n", df.height(), df.width()).into_bytes())
}
