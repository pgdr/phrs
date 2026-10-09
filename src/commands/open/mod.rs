//! Workbook-backed input commands. Unlike CSV commands, these read files rather
//! than standard input and return CSV bytes for the existing stdout pipeline.
mod excel;

use crate::{cli::Invocation, error::PhError};

pub fn run(inv: &Invocation) -> Result<Vec<u8>, PhError> {
    let sheet = inv.option("sheet");
    let sheet_name = inv.option("sheet_name");
    if sheet.is_some() && sheet_name.is_some() {
        return Err(PhError::new(
            "Specify either --sheet or --sheet_name, not both.",
        ));
    }

    let selector = match (sheet, sheet_name) {
        (Some(name), _) => Some(excel::SheetSelector::Name(name)),
        (_, Some(value)) => {
            // Match pandas.read_excel: integer sheet_name values are zero-based
            // worksheet indices, while other values are worksheet names.
            if !value.is_empty() && value.bytes().all(|c| c.is_ascii_digit()) {
                let index = value.parse::<usize>().map_err(|_| {
                    PhError::new(format!("Worksheet index {value} exceeds supported range."))
                })?;
                Some(excel::SheetSelector::Index(index))
            } else if value.starts_with('-') && value[1..].bytes().all(|c| c.is_ascii_digit()) {
                return Err(PhError::new(format!(
                    "Worksheet index {value} must be non-negative."
                )));
            } else {
                Some(excel::SheetSelector::Name(value))
            }
        }
        (None, None) => None,
    };

    match inv.args[0].as_str() {
        "excel" => excel::run(&inv.args[1], selector),
        other => Err(PhError::new(format!("Unknown open format: {other}."))),
    }
}
