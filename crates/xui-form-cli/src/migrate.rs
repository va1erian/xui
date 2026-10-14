#![forbid(unsafe_code)]

//! `xui-form migrate`: convert a format-1 (TOML) form to format 2.

use std::process::ExitCode;

use xui_form::migrate::MigrateError;

use crate::args::{self, Args, Failure, Outcome};

/// Converts the file named, writing format 2 to `--out` or to standard
/// output, and what was dropped to standard error. A form that breaks a rule
/// of format 1 is reported node by node and nothing is written (exit 1).
pub(crate) fn run(mut args: Args) -> Outcome {
    let out = args.option("out")?;
    let Some(file) = args.next_positional() else {
        return Err(Failure::Usage("`migrate` needs a file".to_owned()));
    };
    args.finish()?;
    let text = args::read(file.as_ref())?;
    let migration = match xui_form::migrate::from_v1(&text) {
        Ok(migration) => migration,
        // One line per violation, so each can be found; nothing is written.
        Err(MigrateError::Invalid(problems)) => {
            for problem in &problems {
                eprintln!(
                    "{file}: error: node `{}`: {}",
                    problem.node, problem.message
                );
            }
            return Ok(ExitCode::FAILURE);
        }
        Err(error) => return Err(Failure::Fatal(format!("{file}: {error}"))),
    };
    for note in &migration.notes {
        eprintln!("{file}: note: {note}");
    }
    let converted = migration
        .form
        .to_ron()
        .map_err(|error| Failure::Fatal(format!("{file}: {error}")))?;
    match out {
        Some(out) => args::write(out.as_ref(), &converted)?,
        None => print!("{converted}"),
    }
    Ok(ExitCode::SUCCESS)
}
