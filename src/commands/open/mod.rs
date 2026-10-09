//! Workbook-backed input commands. Unlike CSV commands, these read files rather
//! than standard input and return CSV bytes for the existing stdout pipeline.
mod excel;

use crate::{cli::Invocation, error::PhError};

pub fn run(inv: &Invocation) -> Result<Vec<u8>, PhError> {
    match inv.args[0].as_str() {
        "excel" => excel::run(&inv.args[1], inv.option("sheet")),
        other => Err(PhError::new(format!("Unknown open format: {other}."))),
    }
}
