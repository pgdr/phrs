use crate::{error::PhError, io};
use polars::prelude::*;

/// Mirror ph's slugify_name, including operator words and numeric suffixes.
pub fn slugify_name(original: &str) -> String {
    let mut name = original.to_owned();

    // Python tries float(), then int(), and appends '_' to numeric names.
    let trimmed = original.trim();
    if let Ok(value) = trimmed.parse::<i64>() {
        name = format!("{value}_");
    } else if let Ok(value) = trimmed.parse::<f64>() {
        // Python writes integral float values with a decimal point ("3.0").
        name = if value.fract() == 0.0 && value.is_finite() {
            format!("{value:.1}_")
        } else {
            format!("{value}_")
        };
    }

    if name.is_empty() {
        return "unnamed".to_owned();
    }
    if name == "_" {
        return name;
    }

    // These are captured before operator replacement, just as in Python ph.
    let leading_underscore = name.starts_with('_');
    let trailing_underscore = name.ends_with('_');

    // Order is significant: '**' must be replaced before '*'.
    for (operator, word) in [
        ("+", "plus"),
        ("**", "exponent"),
        ("*", "times"),
        ("-", "minus"),
        ("/", "div"),
        ("%", "modulo"),
        (".", "dot"),
        (",", "comma"),
        ("|", "or"),
        ("&", "and"),
        ("^", "hat"),
    ] {
        name = name.replace(operator, &format!("_{word}_"));
    }

    let mut cleaned = String::new();
    let mut previous_underscore = false;
    for ch in name.trim().to_lowercase().chars() {
        if ch.is_alphanumeric() {
            cleaned.push(ch);
            previous_underscore = false;
        } else if !previous_underscore {
            cleaned.push('_');
            previous_underscore = true;
        }
    }

    let mut result = cleaned.trim_matches('_').to_owned();
    if leading_underscore {
        result.insert(0, '_');
    }
    if trailing_underscore {
        result.push('_');
    }
    result
}

pub fn run(mut df: DataFrame) -> Result<Vec<u8>, PhError> {
    let old: Vec<String> = df
        .get_column_names()
        .iter()
        .map(|n| n.as_str().to_owned())
        .collect();
    let new: Vec<String> = old.iter().map(|n| slugify_name(n)).collect();
    // Use a temporary name for each column to avoid collisions during swaps.
    // Polars will reject final duplicate names, which cannot be represented faithfully.
    for (idx, target) in new.iter().enumerate() {
        if new[..idx].contains(target) {
            return Err(PhError::new(format!(
                "Duplicate slugified column: {target}."
            )));
        }
    }
    let mut tmp = Vec::new();
    for (idx, name) in old.iter().enumerate() {
        let temp = format!("__phrs_internal_{idx}__");
        if old.contains(&temp) || new.contains(&temp) {
            return Err(PhError::new("Internal temporary column name collision."));
        }
        df.rename(name, temp.clone().into())?;
        tmp.push(temp);
    }
    for (name, target) in tmp.iter().zip(&new) {
        df.rename(name, target.clone().into())?;
    }
    io::write_csv(df)
}

#[cfg(test)]
mod tests {
    use super::slugify_name as s;
    #[test]
    fn names() {
        assert_eq!(s("  Stupid column 1"), "stupid_column_1");
        assert_eq!(s("  Jerky-column No. 2"), "jerky_minus_column_no_dot_2");
        assert_eq!(s("4"), "4_");
        assert_eq!(s("3.0"), "3_dot_0_");
        assert_eq!(s("_Hi_"), "_hi_");
        assert_eq!(s("_"), "_");
        assert_eq!(s(""), "unnamed");
        assert_eq!(s("Ålesund City"), "ålesund_city");
        assert_eq!(s("A+B"), "a_plus_b");
        assert_eq!(s("x**2"), "x_exponent_2");
        assert_eq!(s("x*y"), "x_times_y");
        assert_eq!(s("x-y"), "x_minus_y");
        assert_eq!(s("x/y"), "x_div_y");
        assert_eq!(s("x%y"), "x_modulo_y");
        assert_eq!(s("x.y"), "x_dot_y");
        assert_eq!(s("x,y"), "x_comma_y");
        assert_eq!(s("x|y"), "x_or_y");
        assert_eq!(s("x&y"), "x_and_y");
        assert_eq!(s("x^y"), "x_hat_y");
        assert_eq!(s("x***y"), "x_exponent_times_y");
        assert_eq!(s("-2"), "minus_2_");
        assert_eq!(s("3.14"), "3_dot_14_");
        assert_eq!(s("col no.2"), "col_no_dot_2");
        assert_eq!(s("a-b"), "a_minus_b");
    }
}
