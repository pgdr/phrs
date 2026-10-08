use crate::error::PhError;
use polars::prelude::*;
use serde_json::{Number, Value};
use tabulate_rs::{Headers, ShowIndex, TabulateOptions, tabulate};

/// Preserve numeric types instead of passing CSV strings through tabulate's
/// numeric inference. String-valued columns must remain strings.
fn cell(value: AnyValue<'_>) -> Value {
    match value {
        AnyValue::Null => Value::Null,
        AnyValue::Boolean(v) => Value::Bool(v),
        AnyValue::String(v) => Value::String(v.to_owned()),
        AnyValue::StringOwned(v) => Value::String(v.to_string()),
        value => {
            let text = value.to_string();
            // JSON numbers provide tabulate with typed numeric inputs.
            // Special floating point values are strings because JSON cannot
            // represent NaN or infinity.
            if let Ok(i) = text.parse::<i64>() {
                Value::Number(Number::from(i))
            } else if let Ok(u) = text.parse::<u64>() {
                Value::Number(Number::from(u))
            } else if let Ok(f) = text.parse::<f64>() {
                Number::from_f64(f)
                    .map(Value::Number)
                    .unwrap_or_else(|| Value::String(text))
            } else {
                Value::String(text)
            }
        }
    }
}

pub fn render(df: &DataFrame, show_index: bool) -> Result<Vec<u8>, PhError> {
    let cols = df.columns();
    let mut rows: Vec<Vec<Value>> = Vec::with_capacity(df.height());
    for row in 0..df.height() {
        let mut values = Vec::with_capacity(cols.len());
        for col in cols {
            values.push(cell(col.as_materialized_series().get(row)?));
        }
        rows.push(values);
    }

    // Explicit headers avoid consuming a data row, even for empty frames.
    let headers = Headers::Explicit(
        df.get_column_names()
            .iter()
            .map(|s| s.to_string())
            .collect(),
    );
    let string_columns = cols
        .iter()
        .enumerate()
        .filter(|(_, col)| matches!(col.dtype(), DataType::String))
        .map(|(index, _)| index + usize::from(show_index))
        .collect::<Vec<_>>();

    let options = TabulateOptions::new()
        .headers(headers)
        .show_index(if show_index {
            ShowIndex::Always
        } else {
            ShowIndex::Never
        })
        .table_format("simple")
        .missing_value("nan")
        .disable_numparse_columns(string_columns);
    let rendered = tabulate(rows, options).map_err(|e| PhError::new(e.to_string()))?;
    Ok(format!("{rendered}\n").into_bytes())
}
