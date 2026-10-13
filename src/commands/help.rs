//! Human-readable command list and per-command usage, without reading stdin.
use crate::{cli::Invocation, error::PhError};

struct CommandHelp {
    name: &'static str,
    usage: &'static str,
    description: &'static str,
}

const COMMANDS: &[CommandHelp] = &[
    CommandHelp {
        name: "cat",
        usage: "cat [FILE ...] [--axis=0|1]",
        description: "Concatenate input CSV files or standard input.",
    },
    CommandHelp {
        name: "columns",
        usage: "columns [COLUMN ...]",
        description: "List, select, and reorder columns.",
    },
    CommandHelp {
        name: "date",
        usage: "date COLUMN [--format=FORMAT] [--dayfirst=True] [--utc=True]",
        description: "Parse a column as dates.",
    },
    CommandHelp {
        name: "describe",
        usage: "describe",
        description: "Compute descriptive statistics for the input DataFrame.",
    },
    CommandHelp {
        name: "diff",
        usage: "diff [COLUMN ...] [--periods=N] [--axis=0|1]",
        description: "Compute differences between rows or columns.",
    },
    CommandHelp {
        name: "dropna",
        usage: "dropna [--axis=0|1] [--how=any|all] [--thresh=N]",
        description: "Remove rows or columns with missing values.",
    },
    CommandHelp {
        name: "eval",
        usage: "eval EXPR",
        description: "Evaluate an arithmetic expression on columns.",
    },
    CommandHelp {
        name: "fillna",
        usage: "fillna VALUE [--limit=N] | fillna --method=pad|ffill|backfill|bfill [--limit=N]",
        description: "Fill missing values with a scalar or a nearby value.",
    },
    CommandHelp {
        name: "head",
        usage: "head [N]",
        description: "Print the first N rows (default: 10).",
    },
    CommandHelp {
        name: "help",
        usage: "help [COMMAND]",
        description: "List commands or show detailed command usage.",
    },
    CommandHelp {
        name: "max",
        usage: "max [COLUMN ...] [--axis=0|1] [--skipna=True|False]",
        description: "Calculate maximum values.",
    },
    CommandHelp {
        name: "mean",
        usage: "mean [COLUMN ...] [--axis=0|1] [--skipna=True|False] [--numeric_only=True]",
        description: "Calculate arithmetic means.",
    },
    CommandHelp {
        name: "median",
        usage: "median [COLUMN ...] [--axis=0|1] [--skipna=True|False] [--numeric_only=True]",
        description: "Calculate medians.",
    },
    CommandHelp {
        name: "merge",
        usage: "merge LEFT RIGHT [--how=inner|left|right|outer] [--on=COLUMN]",
        description: "Join two input files.",
    },
    CommandHelp {
        name: "min",
        usage: "min [COLUMN ...] [--axis=0|1] [--skipna=True|False]",
        description: "Calculate minimum values.",
    },
    CommandHelp {
        name: "open",
        usage: "open excel FILE [--sheet=NAME] [--sheet_name=INDEX|NAME]",
        description: "Read Excel workbook data as CSV.",
    },
    CommandHelp {
        name: "query",
        usage: "query EXPR",
        description: "Filter rows using a boolean comparison.",
    },
    CommandHelp {
        name: "rename",
        usage: "rename OLD NEW",
        description: "Rename one column.",
    },
    CommandHelp {
        name: "rolling",
        usage: "rolling WINDOW [COLUMN ...] [--how=sum|mean|median|std|min|max] [--min_periods=N] [--center=True] [--ddof=N]",
        description: "Compute rolling-window statistics.",
    },
    CommandHelp {
        name: "round",
        usage: "round COLUMN [DECIMALS] [--decimals=N]",
        description: "Round one numeric column to DECIMALS places (default: 0). Negative decimals round to tens, hundreds, etc. Uses ties-to-even rounding.",
    },
    CommandHelp {
        name: "shape",
        usage: "shape",
        description: "Print the number of rows and columns.",
    },
    CommandHelp {
        name: "show",
        usage: "show [--no-index]",
        description: "Display a human-readable table instead of CSV.",
    },
    CommandHelp {
        name: "slice",
        usage: "slice START:STOP[:STEP]",
        description: "Select rows using Python-style slicing.",
    },
    CommandHelp {
        name: "slugify",
        usage: "slugify",
        description: "Normalize column names to identifiers.",
    },
    CommandHelp {
        name: "sort",
        usage: "sort COLUMN [--ascending=True|False]",
        description: "Sort rows by a column.",
    },
    CommandHelp {
        name: "std",
        usage: "std [COLUMN ...] [--axis=0|1] [--ddof=N] [--skipna=True|False]",
        description: "Calculate standard deviations.",
    },
    CommandHelp {
        name: "strip",
        usage: "strip [COLUMN ...]",
        description: "Trim whitespace in text columns.",
    },
    CommandHelp {
        name: "sum",
        usage: "sum [COLUMN ...] [--axis=0|1] [--skipna=True|False] [--min_count=N]",
        description: "Calculate column or row sums.",
    },
    CommandHelp {
        name: "tail",
        usage: "tail [N]",
        description: "Print the last N rows (default: 10).",
    },
];

pub fn run(inv: &Invocation) -> Result<Vec<u8>, PhError> {
    let result = if let Some(name) = inv.args.first() {
        let info = COMMANDS
            .iter()
            .find(|command| command.name == name)
            .ok_or_else(|| PhError::new(format!("Unknown command {name}.")))?;
        format!("Usage: phrs {}\n\n{}\n", info.usage, info.description)
    } else {
        let mut result = "Usage: phrs COMMAND [ARGS] [--OPTION=VALUE]\n\nCommands:\n".to_owned();
        for command in COMMANDS {
            result.push_str(&format!("  {:<11} {}\n", command.name, command.description));
        }
        result.push_str("\nRun 'phrs help COMMAND' for command-specific usage.\n");
        result
    };
    Ok(result.into_bytes())
}
