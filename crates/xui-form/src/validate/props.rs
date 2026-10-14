#![forbid(unsafe_code)]

//! Checking a widget's property values against the schema.

use super::{Diagnostic, Severity};
use crate::model::Widget;
use crate::schema::Catalog;
use crate::value::ValueType;

/// Reports each property of `widget` whose value its kind does not accept.
pub(super) fn check(widget: &dyn Widget, catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let Some(spec) = catalog.get(widget.kind()) else {
        return;
    };
    for (property, value) in widget.props() {
        if let Some(spec) = spec.property(&property)
            && !spec.accepts(&value)
        {
            out.push(Diagnostic::new(
                Severity::Error,
                Some(widget.name()),
                Some(&property),
                format!("`{property}` must be {}", describe(&spec.ty)),
            ));
        }
    }
}

/// A property type in words, for a diagnostic.
fn describe(ty: &ValueType) -> String {
    match ty {
        ValueType::Int { min: Some(min), .. } => format!("an int of at least {min}"),
        other => format!("a valid {}", other.type_name()),
    }
}
