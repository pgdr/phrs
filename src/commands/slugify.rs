use crate::{error::PhError, io};
use polars::prelude::*;

/// Port of the supplied Python slugify_name, including numeric suffixes,
/// leading/trailing underscores and Unicode-alphanumeric characters.
pub fn slugify_name(original: &str) -> String {
    let mut name = original.to_owned();
    if let Ok(value) = original.parse::<i64>() {
        name = format!("{value}_");
    } else if let Ok(value) = original.parse::<f64>() {
        // Python canonicalizes floats before adding a trailing underscore.
        name = format!("{value}_");
        if value.fract() == 0.0 && value.is_finite() {
            name = format!("{value:.1}_");
        }
    }
    if name.is_empty() {
        return "unnamed".to_owned();
    }
    if name == "_" {
        return name;
    }
    let lead = name.starts_with('_');
    let trail = name.ends_with('_');
    let mut out = String::new();
    let mut prev_underscore = false;
    for ch in name.trim().to_lowercase().chars() {
        if ch.is_alphanumeric() {
            out.push(ch);
            prev_underscore = false;
        } else if !prev_underscore {
            out.push('_');
            prev_underscore = true;
        }
    }
    let mut out = out.trim_matches('_').to_owned();
    if lead {
        out.insert(0, '_');
    }
    if trail {
        out.push('_');
    }
    out
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
        assert_eq!(s("  Jerky-column No. 2"), "jerky_column_no_2");
        assert_eq!(s("4"), "4_");
        assert_eq!(s("3.0"), "3_0_");
        assert_eq!(s("_Hi_"), "_hi_");
        assert_eq!(s("_"), "_");
        assert_eq!(s(""), "unnamed");
        assert_eq!(s("Ålesund City"), "ålesund_city");
    }
}
