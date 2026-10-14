#![forbid(unsafe_code)]

//! Checking a widget's property values against the schema.

use super::{Diagnostic, Severity};
use crate::model::Widget;
use crate::schema::{Access, Catalog};
use crate::value::{Value, ValueType};

/// Reports each property of `widget` whose value its kind does not accept,
/// and the ranges and selections that contradict each other.
pub(super) fn check(widget: &dyn Widget, catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let Some(spec) = catalog.get(widget.kind()) else {
        return;
    };
    let props = widget.props();
    let mut problem = |severity: Severity, property: &str, message: String| {
        out.push(Diagnostic::new(
            severity,
            Some(widget.name()),
            Some(property),
            message,
        ));
    };
    for (property, value) in &props {
        if let Some(spec) = spec.property(property)
            && !spec.accepts(value)
        {
            problem(
                Severity::Error,
                property,
                format!("`{property}` must be {}", describe(&spec.ty)),
            );
        }
    }
    if let (Some(min), Some(max)) = (
        props.get("min").and_then(Value::as_float),
        props.get("max").and_then(Value::as_float),
    ) && min > max
    {
        problem(
            Severity::Error,
            "min",
            format!("`min` ({min}) is greater than `max` ({max})"),
        );
    }
    if let (Some(Value::List(items)), Some(Value::Int(selected))) =
        (props.get("items"), props.get("selected"))
    {
        let beyond = *selected >= 0 && *selected as u64 >= items.len() as u64;
        // The default selection of an empty list is not a mistake.
        if beyond && !(*selected == 0 && items.is_empty()) {
            // A list's items can be replaced at runtime, so a script may
            // still make the selection valid; a combo box's cannot.
            let fixed = spec
                .property("items")
                .is_some_and(|items| items.access == Access::DesignOnly);
            let severity = if fixed {
                Severity::Error
            } else {
                Severity::Warning
            };
            problem(
                severity,
                "selected",
                format!(
                    "`selected` is {selected} but there {}",
                    count(items.len(), "item")
                ),
            );
        }
    }
}

/// `is 1 item`, `are 3 items`, `are no items`: a count in words.
fn count(len: usize, noun: &str) -> String {
    match len {
        0 => format!("are no {noun}s"),
        1 => format!("is 1 {noun}"),
        len => format!("are {len} {noun}s"),
    }
}

/// A property type in words, for a diagnostic.
fn describe(ty: &ValueType) -> String {
    match ty {
        ValueType::Int {
            min: Some(min),
            max: Some(max),
        } => format!("an int from {min} to {max}"),
        ValueType::Int { min: Some(min), .. } => format!("an int of at least {min}"),
        ValueType::Int { max: Some(max), .. } => format!("an int of at most {max}"),
        other => format!("a valid {}", other.type_name()),
    }
}
