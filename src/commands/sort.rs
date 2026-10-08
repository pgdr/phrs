use crate::{cli::Invocation, compat, error::PhError, io};
use polars::prelude::*;
pub fn run(df: DataFrame, inv: &Invocation) -> Result<Vec<u8>, PhError> {
    let column = &inv.args[0];
    compat::columns::require(&df, column)?;
    // Do not silently accept unsupported options: Python behavior needs oracle verification.
    for (key, _) in &inv.kwargs {
        if key != "ascending" {
            return Err(PhError::new(format!("Unknown option --{key}.")));
        }
    }
    if let Some(flag) = inv.flags.first() {
        return Err(PhError::new(format!("Unknown flag --{flag}.")));
    }
    let descending = match inv.option("ascending") {
        None | Some("True" | "true" | "1") => false,
        Some("False" | "false" | "0") => true,
        Some(value) => return Err(PhError::new(format!("Invalid ascending value: {value}."))),
    };
    // pandas sort_values is stable for a single column when kind=stable;
    // missing values are placed last in both ascending and descending order.
    let opts = SortMultipleOptions::default()
        .with_order_descending(descending)
        .with_nulls_last(true)
        .with_maintain_order(true);
    io::write_csv(df.sort([column.as_str()], opts)?)
}
