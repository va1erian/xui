#![forbid(unsafe_code)]

//! `xui-form check`: load and validate forms, one line per problem.

use std::process::ExitCode;

use xui_form::{Catalog, Diagnostic, Severity};

use crate::args::{self, Args, Failure, Outcome};

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
        let text = args::read(&file)?;
        let name = file.display();
        let form = match xui_form::load(&text) {
            Ok(form) => form,
            Err(error) => {
                failed = true;
                println!(
                    "{name}:{}:{}: error: {}",
                    error.line,
                    error.column,
                    describe_load(&error)
                );
                continue;
            }
        };
        let diagnostics = form.validate(&catalog);
        for diagnostic in &diagnostics {
            println!("{name}: {}", describe(diagnostic));
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

/// A load error's message with its suggestion.
fn describe_load(error: &xui_form::LoadError) -> String {
    match &error.suggestion {
        Some(suggestion) => format!("{} (did you mean `{suggestion}`?)", error.message),
        None => error.message.clone(),
    }
}

/// One diagnostic: severity, where, and what.
fn describe(diagnostic: &Diagnostic) -> String {
    let severity = match diagnostic.severity {
        Severity::Error => "error",
        Severity::Warning => "warning",
    };
    let place = match (&diagnostic.node, &diagnostic.property) {
        (Some(node), _) => format!(" `{node}`:"),
        (None, Some(property)) => format!(" `{property}`:"),
        (None, None) => String::new(),
    };
    format!("{severity}:{place} {}", diagnostic.message)
}
