use crate::{compat::describe::Describable, error::PhError, io};
use polars::prelude::*;

pub fn run(df: DataFrame) -> Result<Vec<u8>, PhError> {
    let result = df.describe(None).map_err(|e| PhError::new(e.to_string()))?;

    io::write_csv(result)
}
