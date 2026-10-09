pub mod cli;
mod commands;
mod compat;
pub mod error;
mod io;

use std::ffi::OsString;
use std::io::{Read, Write};
use std::process::ExitCode;

pub const PROGRAM_NAME: &str = "phrs";
pub const COMPAT_PH_VERSION: &str = "1.1.7";

pub fn main(args: impl IntoIterator<Item = OsString>) -> ExitCode {
    match run(args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("{}", err.message);
            ExitCode::from(err.code)
        }
    }
}

pub fn run(args: impl IntoIterator<Item = OsString>) -> Result<(), error::PhError> {
    let invocation = cli::parse(args)?;
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    execute_io(invocation, stdin.lock(), stdout.lock())
}

/// Execute against arbitrary streams; useful for library clients and unit tests.
pub fn execute_io(
    invocation: cli::Invocation,
    mut input: impl Read,
    mut output: impl Write,
) -> Result<(), error::PhError> {
    commands::validate(&invocation)?;
    let mut bytes = Vec::new();
    if invocation.command != "open"
        && invocation.command != "merge"
        && !(invocation.command == "cat" && !invocation.args.is_empty())
    {
        input.read_to_end(&mut bytes)?;
    }
    let result = commands::execute(&invocation, &bytes)?;
    output.write_all(&result)?;
    Ok(())
}
