use crate::{cli::Invocation, error::PhError, io};
use polars::prelude::*;

pub fn run(inv: &Invocation, stdin: &[u8]) -> Result<Vec<u8>, PhError> {
    let axis = inv.option("axis").unwrap_or("index");
    if axis != "index" && axis != "columns" {
        return Err(PhError::new(format!("Unknown axis command '{axis}'")));
    }
    if inv.args.is_empty() {
        return io::write_csv(io::read_csv(stdin)?);
    }
    let frames: Vec<DataFrame> = inv
        .args
        .iter()
        .map(|name| {
            std::fs::read(name)
                .map_err(PhError::from)
                .and_then(|buf| io::read_csv(&buf))
        })
        .collect::<Result<_, _>>()?;
    let result = if axis == "columns" {
        horizontal(&frames)?
    } else {
        vertical(&frames)?
    };
    io::write_csv(result)
}

fn vertical(frames: &[DataFrame]) -> Result<DataFrame, PhError> {
    let first = &frames[0];
    let names: Vec<String> = first
        .get_column_names()
        .iter()
        .map(|n| n.to_string())
        .collect();
    // pandas aligns vertical concatenation by column name, and takes the union of columns.
    let mut union = names;
    for df in frames.iter().skip(1) {
        for name in df.get_column_names() {
            if !union.iter().any(|n| n == name.as_str()) {
                union.push(name.to_string());
            }
        }
    }
    let mut types = Vec::with_capacity(union.len());
    for name in &union {
        let mut dtype: Option<DataType> = None;
        let mut absent = false;
        for df in frames {
            match df.column(name) {
                Ok(col) => {
                    dtype = Some(match dtype {
                        None => col.dtype().clone(),
                        Some(ref old) if old == col.dtype() => old.clone(),
                        Some(ref old)
                            if matches!(old, DataType::String)
                                || matches!(col.dtype(), DataType::String) =>
                        {
                            DataType::String
                        }
                        Some(ref old)
                            if matches!(old, DataType::Float32 | DataType::Float64)
                                || matches!(col.dtype(), DataType::Float32 | DataType::Float64) =>
                        {
                            DataType::Float64
                        }
                        Some(_) => DataType::Int64,
                    });
                }
                Err(_) => absent = true,
            }
        }
        let mut ty = dtype.unwrap_or(DataType::String);
        if absent
            && matches!(
                ty,
                DataType::Int8
                    | DataType::Int16
                    | DataType::Int32
                    | DataType::Int64
                    | DataType::UInt8
                    | DataType::UInt16
                    | DataType::UInt32
                    | DataType::UInt64
            )
        {
            ty = DataType::Float64;
        }
        types.push(ty);
    }
    let mut result: Option<DataFrame> = None;
    for df in frames {
        let mut columns = Vec::with_capacity(union.len());
        for (name, ty) in union.iter().zip(types.iter()) {
            let col = match df.column(name) {
                Ok(c) => c.cast(ty)?,
                Err(_) => Column::full_null(name.clone().into(), df.height(), ty),
            };
            columns.push(col);
        }
        let aligned = DataFrame::new_infer_height(columns)?;
        if let Some(acc) = &mut result {
            acc.vstack_mut(&aligned)?;
        } else {
            result = Some(aligned);
        }
    }
    Ok(result.expect("nonempty file list"))
}

fn horizontal(frames: &[DataFrame]) -> Result<DataFrame, PhError> {
    let max_rows = frames.iter().map(DataFrame::height).max().unwrap_or(0);
    let mut columns = Vec::new();
    for df in frames {
        for original in df.columns() {
            if columns.iter().any(|c: &Column| c.name() == original.name()) {
                return Err(PhError::new(format!(
                    "Duplicate column '{}' in column-wise concat (Polars requires unique names).",
                    original.name()
                )));
            }
            let mut column = original.clone();
            if column.len() < max_rows {
                let padding = Column::full_null(
                    column.name().clone(),
                    max_rows - column.len(),
                    column.dtype(),
                );
                column.append(&padding)?;
                if column.null_count() > 0
                    && matches!(
                        column.dtype(),
                        DataType::Int64 | DataType::Int32 | DataType::UInt64 | DataType::UInt32
                    )
                {
                    column = column.cast(&DataType::Float64)?;
                }
            }
            columns.push(column);
        }
    }
    Ok(DataFrame::new_infer_height(columns)?)
}
