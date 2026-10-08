use crate::{cli::Invocation, compat, error::PhError, io};
use polars::prelude::*;

pub fn run(df: DataFrame, inv: &Invocation) -> Result<Vec<u8>, PhError> {
    if inv.args.is_empty() {
        let mut out = b"columns\n".to_vec();
        for col in df.get_column_names() {
            out.extend_from_slice(col.as_str().as_bytes());
            out.push(b'\n');
        }
        return Ok(out);
    }
    for name in &inv.args {
        compat::columns::require(&df, name)?;
    }
    let names: Vec<&str> = inv.args.iter().map(String::as_str).collect();
    io::write_csv(df.select(names)?)
}
