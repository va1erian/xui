#![forbid(unsafe_code)]

//! `xui-form migrate`: convert a format-1 (TOML) form to format 2.

use std::process::ExitCode;

use crate::args::{self, Args, Failure, Outcome};

/// Converts the file named, writing format 2 to `--out` or to standard
/// output, and what was dropped to standard error.
pub(crate) fn run(mut args: Args) -> Outcome {
    let out = args.option("out")?;
    let Some(file) = args.next_positional() else {
        return Err(Failure::Usage("`migrate` needs a file".to_owned()));
    };
    args.finish()?;
    let text = args::read(file.as_ref())?;
    let migration = xui_form::migrate::from_v1(&text)
        .map_err(|error| Failure::Fatal(format!("{file}: {error}")))?;
    for note in &migration.notes {
        eprintln!("{file}: note: {note}");
    }
    let converted = migration.form.to_ron();
    match out {
        Some(out) => args::write(out.as_ref(), &converted)?,
        None => print!("{converted}"),
    }
    Ok(ExitCode::SUCCESS)
}
