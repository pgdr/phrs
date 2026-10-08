use crate::{cli::Invocation, compat, error::PhError, io};
use polars::prelude::*;

pub fn run(mut df: DataFrame, inv: &Invocation) -> Result<Vec<u8>, PhError> {
    // Without column names, trim all string columns; otherwise only requested columns.
    let names: Vec<String> = if inv.args.is_empty() {
        df.columns()
            .iter()
            .filter(|c| matches!(c.dtype(), DataType::String))
            .map(|c| c.name().as_str().to_owned())
            .collect()
    } else {
        inv.args.clone()
    };
    for name in &names {
        compat::columns::require(&df, name)?;
        let col = df.column(name)?;
        if !matches!(col.dtype(), DataType::String) {
            continue;
        }
        let values: Vec<Option<String>> = col
            .str()?
            .iter()
            .map(|v| v.map(|s| s.trim().to_owned()))
            .collect();
        df.replace(name, Series::new(name.as_str().into(), values).into())?;
    }
    io::write_csv(df)
}
