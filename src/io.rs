use crate::error::PhError;
use polars::prelude::*;
use std::io::Cursor;

pub fn read_csv(data: &[u8]) -> Result<DataFrame, PhError> {
    let cursor = Cursor::new(data.to_vec());
    let mut df = CsvReadOptions::default()
        .with_has_header(true)
        .with_infer_schema_length(None)
        .into_reader_with_file_handle(cursor)
        .finish()?;

    // pandas promotes nullable integer columns to float64 on CSV import.
    let names: Vec<_> = df
        .columns()
        .iter()
        .filter(|col| {
            col.null_count() > 0
                && matches!(
                    col.dtype(),
                    DataType::Int8
                        | DataType::Int16
                        | DataType::Int32
                        | DataType::Int64
                        | DataType::UInt8
                        | DataType::UInt16
                        | DataType::UInt32
                        | DataType::UInt64
                )
        })
        .map(|col| col.name().clone())
        .collect();
    for name in names {
        let col = df.column(name.as_str())?.cast(&DataType::Float64)?;
        df.with_column(col)?;
    }
    Ok(df)
}

pub fn write_csv(mut frame: DataFrame) -> Result<Vec<u8>, PhError> {
    let mut out = Vec::new();
    CsvWriter::new(&mut out)
        .include_header(true)
        .finish(&mut frame)?;
    Ok(out)
}
