#![forbid(unsafe_code)]

//! `xui-form check`: load and validate forms, one line per problem.

use std::process::ExitCode;

use xui_form::{Catalog, Diagnostic};

use crate::args::{Args, Failure, Outcome};
use crate::report;

/// Checks every file named; exits 1 when any has an error.
pub(crate) fn run(mut args: Args) -> Outcome {
    let files = args.positionals();
    args.finish()?;
    if files.is_empty() {
        return Err(Failure::Usage("`check` needs at least one file".to_owned()));
    }
    let catalog = Catalog::xui();
    let mut failed = false;
    for file in files {
        let name = file.display();
        // An unreadable file is that file's error, not the end of the run.
        let text = match std::fs::read_to_string(&file) {
            Ok(text) => text,
            Err(error) => {
                failed = true;
                println!("{name}: error: {error}");
                continue;
            }
        };
        let form = match xui_form::load(&text) {
            Ok(form) => form,
            Err(error) => {
                failed = true;
                println!("{}", report::load_error(&name, &error));
                continue;
            }
        };
        let diagnostics = form.validate(&catalog);
        for diagnostic in &diagnostics {
            println!("{}", report::diagnostic(&name, diagnostic));
        }
        failed |= diagnostics.iter().any(Diagnostic::is_error);
        if diagnostics.is_empty() {
            println!("{name}: ok");
        }
    }
    Ok(if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    })
}
