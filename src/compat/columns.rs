use crate::error::PhError;
use polars::prelude::DataFrame;

pub fn require(df: &DataFrame, column: &str) -> Result<(), PhError> {
    if df
        .get_column_names()
        .iter()
        .any(|name| name.as_str() == column)
    {
        Ok(())
    } else {
        Err(PhError::new(format!("Unknown column {column}.")))
    }
}
