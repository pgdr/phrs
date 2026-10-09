//! Recognize pandas read_csv's default NA spellings as well as Polars nulls
//! and IEEE floating-point NaNs. Keep this local to NA commands so that
//! existing unrelated commands retain their current CSV ingestion behavior.
use polars::prelude::AnyValue;

fn na_string(value: &str) -> bool {
    matches!(
        value,
        "" | "#N/A"
            | "#N/A N/A"
            | "#NA"
            | "-1.#IND"
            | "-1.#QNAN"
            | "-NaN"
            | "-nan"
            | "1.#IND"
            | "1.#QNAN"
            | "<NA>"
            | "N/A"
            | "NA"
            | "NULL"
            | "NaN"
            | "None"
            | "n/a"
            | "nan"
            | "null"
    )
}

pub fn is_missing(value: AnyValue<'_>) -> bool {
    match value {
        AnyValue::Null => true,
        AnyValue::Float64(value) => value.is_nan(),
        AnyValue::Float32(value) => value.is_nan(),
        AnyValue::String(value) => na_string(value),
        AnyValue::StringOwned(value) => na_string(value.as_str()),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pandas_missing_literals() {
        for value in ["NaN", "nan", "NA", "N/A", "None", "<NA>", "NULL", "#N/A"] {
            assert!(is_missing(AnyValue::String(value)), "{value}");
        }
        for value in ["nanosecond", "none", "n.a.", "Infinity", "NA ", "0"] {
            assert!(!is_missing(AnyValue::String(value)), "{value}");
        }
        assert!(is_missing(AnyValue::Null));
        assert!(is_missing(AnyValue::Float64(f64::NAN)));
        assert!(!is_missing(AnyValue::Float64(f64::INFINITY)));
    }
}
