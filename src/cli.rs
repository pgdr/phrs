use crate::error::PhError;
use std::ffi::OsString;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Invocation {
    pub command: String,
    pub args: Vec<String>,
    pub kwargs: Vec<(String, String)>,
    pub flags: Vec<String>,
}

pub fn parse(argv: impl IntoIterator<Item = OsString>) -> Result<Invocation, PhError> {
    let mut argv = argv.into_iter();
    let _program = argv.next();
    let items: Vec<String> = argv
        .map(|a| {
            a.into_string()
                .map_err(|_| PhError::new("Non-UTF-8 argument."))
        })
        .collect::<Result<_, _>>()?;
    let command = items.first().cloned().unwrap_or_default();
    let mut out = Invocation {
        command,
        args: Vec::new(),
        kwargs: Vec::new(),
        flags: Vec::new(),
    };
    for arg in items.into_iter().skip(1) {
        if let Some(opt) = arg.strip_prefix("--") {
            if let Some((key, value)) = opt.split_once('=') {
                out.kwargs.push((key.to_owned(), value.to_owned()));
            } else {
                out.flags.push(opt.to_owned());
            }
        } else {
            out.args.push(arg);
        }
    }
    Ok(out)
}

impl Invocation {
    pub fn option(&self, key: &str) -> Option<&str> {
        self.kwargs
            .iter()
            .rev()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }
    pub fn flag(&self, key: &str) -> bool {
        self.flags.iter().any(|f| f == key)
    }
    pub fn no_options(&self) -> Result<(), PhError> {
        if let Some((key, _)) = self.kwargs.first() {
            return Err(PhError::new(format!("Unknown option --{key}.")));
        }
        if let Some(key) = self.flags.first() {
            return Err(PhError::new(format!("Unknown flag --{key}.")));
        }
        Ok(())
    }
    pub fn arity(&self, min: usize, max: usize) -> Result<(), PhError> {
        if self.args.len() < min || self.args.len() > max {
            return Err(PhError::new(format!(
                "Invalid number of arguments for {}.",
                self.command
            )));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parse_preserves_raw_values() {
        let inv = parse(
            ["phrs", "sort", "a", "--ascending=False", "--stable"]
                .into_iter()
                .map(OsString::from),
        )
        .unwrap();
        assert_eq!(inv.command, "sort");
        assert_eq!(inv.args, ["a"]);
        assert_eq!(inv.kwargs, [("ascending".to_string(), "False".to_string())]);
        assert_eq!(inv.flags, ["stable"]);
    }
}
