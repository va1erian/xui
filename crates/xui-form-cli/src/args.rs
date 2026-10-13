#![forbid(unsafe_code)]

//! The command line, read in order: positionals and `--flag [value]`
//! options.

use std::collections::VecDeque;
use std::path::PathBuf;

/// Why a command stopped.
pub(crate) enum Failure {
    /// The command line was wrong: print the message and the usage.
    Usage(String),
    /// The command could not do its work: print the message.
    Fatal(String),
}

/// What a command returns.
pub(crate) type Outcome = Result<std::process::ExitCode, Failure>;

/// The arguments after the program name.
pub(crate) struct Args(VecDeque<String>);

impl Args {
    /// The process's arguments.
    pub(crate) fn from_env() -> Args {
        Args(std::env::args().skip(1).collect())
    }

    /// The next argument that is not an option, removed.
    pub(crate) fn next_positional(&mut self) -> Option<String> {
        let index = self.0.iter().position(|arg| !arg.starts_with("--"))?;
        self.0.remove(index)
    }

    /// Every remaining positional, removed.
    pub(crate) fn positionals(&mut self) -> Vec<PathBuf> {
        std::iter::from_fn(|| self.next_positional())
            .map(PathBuf::from)
            .collect()
    }

    /// Whether `--name` was given, removed.
    pub(crate) fn flag(&mut self, name: &str) -> bool {
        let flag = format!("--{name}");
        match self.0.iter().position(|arg| *arg == flag) {
            Some(index) => {
                self.0.remove(index);
                true
            }
            None => false,
        }
    }

    /// The value after `--name`, removed with it.
    pub(crate) fn option(&mut self, name: &str) -> Result<Option<String>, Failure> {
        let flag = format!("--{name}");
        let Some(index) = self.0.iter().position(|arg| *arg == flag) else {
            return Ok(None);
        };
        self.0.remove(index);
        match self.0.remove(index) {
            Some(value) => Ok(Some(value)),
            None => Err(Failure::Usage(format!("`{flag}` needs a value"))),
        }
    }

    /// Fails on anything left over: an unknown option or an extra argument.
    pub(crate) fn finish(self) -> Result<(), Failure> {
        match self.0.front() {
            Some(extra) => Err(Failure::Usage(format!("unexpected argument `{extra}`"))),
            None => Ok(()),
        }
    }
}

/// Reads `path`, failing with a message that names it.
pub(crate) fn read(path: &std::path::Path) -> Result<String, Failure> {
    std::fs::read_to_string(path)
        .map_err(|error| Failure::Fatal(format!("{}: {error}", path.display())))
}

/// Writes `text` to `path`, failing with a message that names it.
pub(crate) fn write(path: &std::path::Path, text: &str) -> Result<(), Failure> {
    std::fs::write(path, text)
        .map_err(|error| Failure::Fatal(format!("{}: {error}", path.display())))
}

/// Loads the form at `path`, failing with its located error.
pub(crate) fn load(path: &std::path::Path) -> Result<xui_form::Form, Failure> {
    let text = read(path)?;
    xui_form::load(&text).map_err(|error| Failure::Fatal(format!("{}:{error}", path.display())))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Args {
        Args(list.iter().map(|arg| (*arg).to_owned()).collect())
    }

    #[test]
    fn options_and_positionals_come_out_in_any_order() {
        let mut parsed = args(&["--out", "dir", "form.lfm", "--report"]);
        assert!(parsed.flag("report"));
        assert_eq!(parsed.option("out").ok().flatten().as_deref(), Some("dir"));
        assert_eq!(parsed.positionals(), [PathBuf::from("form.lfm")]);
        assert!(parsed.finish().is_ok());
    }

    #[test]
    fn a_leftover_option_is_a_usage_error() {
        let mut parsed = args(&["form.lfm", "--colour"]);
        let _ = parsed.positionals();
        assert!(matches!(parsed.finish(), Err(Failure::Usage(_))));
        assert!(matches!(
            args(&["--out"]).option("out"),
            Err(Failure::Usage(_))
        ));
    }
}
