pub mod cat;
pub mod columns;
pub mod date;
pub mod diff;
pub mod eval;
pub mod head;
pub mod merge;
pub mod query;
pub mod rename;
pub mod shape;
pub mod show;
pub mod slugify;
pub mod sort;
pub mod strip;
pub mod tail;

use crate::{cli::Invocation, error::PhError, io};

pub fn validate(inv: &Invocation) -> Result<(), PhError> {
    match inv.command.as_str() {
        "cat" => {
            for (key, _) in &inv.kwargs {
                if key != "axis" {
                    return Err(PhError::new(format!("Unknown option --{key}.")));
                }
            }
            if let Some(flag) = inv.flags.first() {
                return Err(PhError::new(format!("Unknown flag --{flag}.")));
            }
            Ok(())
        }
        "merge" => {
            inv.arity(2, 2)?;
            for (key, _) in &inv.kwargs {
                if !["how", "on", "left", "right"].contains(&key.as_str()) {
                    return Err(PhError::new(format!("Unknown option --{key}.")));
                }
            }
            if let Some(flag) = inv.flags.first() {
                return Err(PhError::new(format!("Unknown flag --{flag}.")));
            }
            Ok(())
        }
        "columns" => inv.no_options(),
        "date" => {
            inv.arity(1, 1)?;
            if let Some((key, _)) = inv
                .kwargs
                .iter()
                .find(|(key, _)| !matches!(key.as_str(), "format" | "dayfirst" | "utc"))
            {
                return Err(PhError::new(format!("Unknown option --{key}.")));
            }
            if let Some(key) = inv
                .flags
                .iter()
                .find(|key| !matches!(key.as_str(), "dayfirst" | "utc"))
            {
                return Err(PhError::new(format!("Unknown flag --{key}.")));
            }
            Ok(())
        }

        "diff" => {
            if let Some((key, _)) = inv
                .kwargs
                .iter()
                .find(|(key, _)| key != "periods" && key != "axis")
            {
                return Err(PhError::new(format!("Unknown option --{key}.")));
            }
            if let Some(flag) = inv.flags.first() {
                return Err(PhError::new(format!("Unknown flag --{flag}.")));
            }
            Ok(())
        }

        "head" | "tail" => {
            inv.no_options()?;
            inv.arity(0, 1)
        }
        "rename" => {
            inv.no_options()?;
            inv.arity(2, 2)
        }
        "sort" => {
            inv.arity(1, 1)?;
            Ok(())
        }
        "show" => {
            inv.arity(0, 0)?;
            if let Some((key, _)) = inv.kwargs.first() {
                return Err(PhError::new(format!("Unknown option --{key}.")));
            }
            if let Some(flag) = inv.flags.iter().find(|flag| flag.as_str() != "no-index") {
                return Err(PhError::new(format!("Unknown flag --{flag}.")));
            }
            Ok(())
        }
        "shape" => {
            inv.no_options()?;
            inv.arity(0, 0)
        }
        "query" | "eval" => {
            inv.no_options()?;
            inv.arity(1, 1)
        }
        "strip" => inv.no_options(),
        "slugify" => {
            inv.no_options()?;
            inv.arity(0, 0)
        }
        _ => Err(PhError::new(format!(
            "Unknown command {}.\nUsage: ph command [args]",
            inv.command
        ))),
    }
}

pub fn execute(inv: &Invocation, data: &[u8]) -> Result<Vec<u8>, PhError> {
    if inv.command == "cat" {
        return cat::run(inv, data);
    }
    if inv.command == "merge" {
        return merge::run(inv);
    }
    let df = io::read_csv(data)?;
    match inv.command.as_str() {
        "columns" => columns::run(df, inv),
        "date" => date::run(df, inv),
        "diff" => diff::run(df, inv),
        "head" => head::run(df, inv),
        "tail" => tail::run(df, inv),
        "rename" => rename::run(df, inv),
        "sort" => sort::run(df, inv),
        "shape" => shape::run(df),
        "show" => show::run(df, inv),
        "query" => query::run(df, inv),
        "eval" => eval::run(df, inv),
        "strip" => strip::run(df, inv),
        "slugify" => slugify::run(df),
        _ => unreachable!("validated before CSV input"),
    }
}

pub fn count_arg(inv: &Invocation, default: i64) -> Result<i64, PhError> {
    match inv.args.first() {
        None => Ok(default),
        Some(v) => v
            .parse::<i64>()
            .map_err(|_| PhError::new(format!("Invalid row count: {v}."))),
    }
}
