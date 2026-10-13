#![forbid(unsafe_code)]

//! `xui-form schema --json`: the catalog, for agents and editors.

use std::process::ExitCode;

use xui_form::Catalog;

use crate::args::{Args, Failure, Outcome};

/// Prints the catalog as pretty JSON: every widget with its properties,
/// events and fields, the layouts, the common properties and the layout
/// and form fields.
pub(crate) fn run(mut args: Args) -> Outcome {
    if !args.flag("json") {
        return Err(Failure::Usage(
            "`schema` prints JSON: pass `--json`".to_owned(),
        ));
    }
    args.finish()?;
    let json = serde_json::to_string_pretty(&Catalog::xui())
        .map_err(|error| Failure::Fatal(error.to_string()))?;
    println!("{json}");
    Ok(ExitCode::SUCCESS)
}
