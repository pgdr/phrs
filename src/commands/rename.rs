use crate::{cli::Invocation, compat, error::PhError, io};
use polars::prelude::*;
pub fn run(mut df: DataFrame, inv: &Invocation) -> Result<Vec<u8>, PhError> {
    compat::columns::require(&df, &inv.args[0])?;
    df.rename(&inv.args[0], (&inv.args[1]).into())?;
    io::write_csv(df)
}
