use crate::{cli::Invocation, error::PhError, io};
use polars::prelude::*;

pub fn run(inv: &Invocation) -> Result<Vec<u8>, PhError> {
    let how = inv.option("how").unwrap_or("inner");
    let join_type = match how {
        "inner" => JoinType::Inner,
        "left" => JoinType::Left,
        "right" => JoinType::Right,
        "outer" => JoinType::Full,
        _ => {
            return Err(PhError::new(format!(
                "Unknown merge --how={}, must be one of ('left', 'right', 'outer', 'inner')",
                how
            )));
        }
    };
    let left_df = io::read_csv(&std::fs::read(&inv.args[0])?)?;
    let right_df = io::read_csv(&std::fs::read(&inv.args[1])?)?;
    let on = inv.option("on");
    let left = inv.option("left");
    let right = inv.option("right");
    if left.is_some() != right.is_some() {
        return Err(PhError::new(format!(
            "Specify columns in both files.  left was {}, right was {}",
            left.unwrap_or("None"),
            right.unwrap_or("None")
        )));
    }
    if on.is_some() && left.is_some() {
        return Err(PhError::new("Cannot specify both --on and --left/--right."));
    }
    let (left_keys, right_keys): (Vec<String>, Vec<String>) = if let (Some(l), Some(r)) =
        (left, right)
    {
        (vec![l.into()], vec![r.into()])
    } else if let Some(key) = on {
        (vec![key.into()], vec![key.into()])
    } else {
        let common: Vec<String> = left_df
            .get_column_names()
            .iter()
            .filter(|name| right_df.column(name.as_str()).is_ok())
            .map(|name| name.to_string())
            .collect();
        if common.is_empty() {
            return Err(PhError::new(
                "No common columns to perform merge on.  Merge options: on, or: left=None, right=None.",
            ));
        }
        (common.clone(), common)
    };
    for key in &left_keys {
        left_df.column(key)?;
    }
    for key in &right_keys {
        right_df.column(key)?;
    }

    // pandas suffixes overlapping non-key columns with _x and _y.
    let mut lhs = left_df;
    let mut rhs = right_df;
    let overlaps: Vec<String> = lhs
        .get_column_names()
        .iter()
        .filter(|name| {
            rhs.column(name.as_str()).is_ok()
                && !left_keys
                    .iter()
                    .zip(right_keys.iter())
                    .any(|(l, r)| l == name.as_str() && r == name.as_str())
        })
        .map(|name| name.to_string())
        .collect();
    for name in &overlaps {
        lhs.rename(name, format!("{name}_x").into())?;
        rhs.rename(name, format!("{name}_y").into())?;
    }
    // Rename can affect differently named join keys when they overlap another column.
    let left_keys: Vec<String> = left_keys
        .iter()
        .map(|n| {
            if overlaps.contains(n) {
                format!("{n}_x")
            } else {
                n.clone()
            }
        })
        .collect();
    let right_keys: Vec<String> = right_keys
        .iter()
        .map(|n| {
            if overlaps.contains(n) {
                format!("{n}_y")
            } else {
                n.clone()
            }
        })
        .collect();
    let left_expr: Vec<Expr> = left_keys.iter().map(col).collect();
    let right_expr: Vec<Expr> = right_keys.iter().map(col).collect();
    let mut args = JoinArgs::new(join_type);
    args.nulls_equal = true; // pandas joins missing keys to one another.
    args.maintain_order = MaintainOrderJoin::LeftRight;
    args.coalesce = JoinCoalesce::CoalesceColumns;
    let mut result = lhs
        .lazy()
        .join(rhs.lazy(), left_expr, right_expr, args)
        .collect()?;
    // A pandas outer join sorts its join keys; inner and left joins retain input order.
    if how == "outer" {
        result = result.sort(
            left_keys.iter().map(String::as_str).collect::<Vec<_>>(),
            SortMultipleOptions::default().with_nulls_last(true),
        )?;
    }
    io::write_csv(result)
}
