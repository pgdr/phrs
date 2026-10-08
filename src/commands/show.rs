use crate::{cli::Invocation, compat::table, error::PhError};
use polars::prelude::DataFrame;

/// Print a DataFrame in the python-tabulate `simple` format, with headers and index.
pub fn run(df: DataFrame, inv: &Invocation) -> Result<Vec<u8>, PhError> {
    table::render(&df, !inv.flag("no-index"))
}
