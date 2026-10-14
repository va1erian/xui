#![forbid(unsafe_code)]

//! Validation of a [`Form`] against a [`Catalog`].
//!
//! Loading already rejects unknown kinds and fields and mistyped values.
//! Validation reports, all at once, what a well-typed tree can still get
//! wrong: names that are not identifiers or are used twice, control arrays
//! that clash or are too large, values out of range, trees past the format's
//! limits and layout fields a node's parent ignores.

use crate::model::Form;
use crate::schema::Catalog;

mod names;
mod place;
mod props;

/// The names a widget cannot have: a script reaches the form itself by
/// `form`, so a control of that name could not be reached.
pub const RESERVED_NAMES: [&str; 1] = ["form"];

/// How serious a [`Diagnostic`] is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Severity {
    /// The form cannot be built correctly.
    Error,
    /// The form is usable but something is suspicious.
    Warning,
}

/// One problem found in a form.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diagnostic {
    /// How serious the problem is.
    pub severity: Severity,
    /// The widget the problem is in, if it has a name.
    pub node: Option<String>,
    /// The property or field the problem is about, if any.
    pub property: Option<String>,
    /// A human-readable description.
    pub message: String,
}

impl Diagnostic {
    fn new(
        severity: Severity,
        node: Option<&str>,
        property: Option<&str>,
        message: String,
    ) -> Self {
        Diagnostic {
            severity,
            node: node.filter(|name| !name.is_empty()).map(str::to_owned),
            property: property.map(str::to_owned),
            message,
        }
    }

    /// Whether this is an error.
    pub fn is_error(&self) -> bool {
        self.severity == Severity::Error
    }
}

impl Form {
    /// Validates the form against `catalog`, returning every problem found.
    ///
    /// The scan does not recurse, so a tree of any depth is validated (one
    /// past [`MAX_DEPTH`](crate::MAX_DEPTH) is itself an error), and a
    /// control array's size is never enumerated.
    pub fn validate(&self, catalog: &Catalog) -> Vec<Diagnostic> {
        let mut out = Vec::new();
        if !self.name.is_empty() && !is_identifier(&self.name) {
            out.push(Diagnostic::new(
                Severity::Error,
                None,
                Some("name"),
                format!("`{}` is not a valid form name", self.name),
            ));
        }
        out.extend(crate::model::breaches(self).into_iter().map(|breach| {
            Diagnostic::new(
                Severity::Error,
                breach.node.as_deref(),
                Some(breach.property),
                breach.message,
            )
        }));
        let mut names = names::Names::default();
        self.root.walk(&mut |node, parent| {
            place::check_place(node, parent, &mut out);
            place::check_layout(node, &mut out);
            place::check_tabs(node, &mut out);
            if let Some(widget) = node.widget() {
                names.check(widget, &mut out);
                props::check(widget, catalog, &mut out);
            }
        });
        out
    }
}

/// Whether `name` is an identifier: a letter or `_`, then letters, digits
/// and `_`.
pub(crate) fn is_identifier(name: &str) -> bool {
    let mut characters = name.chars();
    match characters.next() {
        Some(first) if first.is_ascii_alphabetic() || first == '_' => {}
        _ => return false,
    }
    characters.all(|character| character.is_ascii_alphanumeric() || character == '_')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifiers_are_checked() {
        assert!(is_identifier("hello_button"));
        assert!(is_identifier("_x1"));
        assert!(!is_identifier(""));
        assert!(!is_identifier("1bad"));
        assert!(!is_identifier("has space"));
    }
}
