#![forbid(unsafe_code)]

//! `xui-form fmt`: rewrite forms in the canonical layout.

use std::process::ExitCode;

use crate::args::{self, Args, Failure, Outcome};

/// Formats every file named in place, or with `--check` only lists the ones
/// that are not canonical (exiting 1 when there are any).
pub(crate) fn run(mut args: Args) -> Outcome {
    let check = args.flag("check");
    let files = args.positionals();
    args.finish()?;
    if files.is_empty() {
        return Err(Failure::Usage("`fmt` needs at least one file".to_owned()));
    }
    let mut unformatted = false;
    for file in files {
        let text = args::read(&file)?;
        let formatted = xui_form::format(&text)
            .map_err(|error| Failure::Fatal(format!("{}:{error}", file.display())))?;
        if formatted == text {
            continue;
        }
        unformatted = true;
        if check {
            println!("{}: not formatted", file.display());
        } else {
            args::write(&file, &formatted)?;
            println!("{}: formatted", file.display());
        }
    }
    Ok(if check && unformatted {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    })
}
