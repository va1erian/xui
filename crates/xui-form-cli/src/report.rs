#![forbid(unsafe_code)]

//! The one-line forms of what `check` and `render` report about a form.

use std::fmt::Display;

use xui_form::{Diagnostic, LoadError, Severity};

/// A load error: `file:line:col: error: message (did you mean `x`?)`.
pub(crate) fn load_error(file: &dyn Display, error: &LoadError) -> String {
    let suggestion = match &error.suggestion {
        Some(suggestion) => format!(" (did you mean `{suggestion}`?)"),
        None => String::new(),
    };
    format!(
        "{file}:{}:{}: error: {}{suggestion}",
        error.line, error.column, error.message
    )
}

/// A diagnostic: `file: error: `node`: message`, or `warning:` for a warning.
pub(crate) fn diagnostic(file: &dyn Display, diagnostic: &Diagnostic) -> String {
    let severity = match diagnostic.severity {
        Severity::Error => "error",
        Severity::Warning => "warning",
    };
    let place = match (&diagnostic.node, &diagnostic.property) {
        (Some(node), _) => format!(" `{node}`:"),
        (None, Some(property)) => format!(" `{property}`:"),
        (None, None) => String::new(),
    };
    format!("{file}: {severity}:{place} {}", diagnostic.message)
}
