#![forbid(unsafe_code)]

//! Validation of a [`Form`] against a [`Catalog`].
//!
//! Loading already rejects unknown kinds and fields and mistyped values.
//! Validation reports, all at once, what a well-typed tree can still get
//! wrong: names that are not identifiers or are used twice, control arrays
//! that clash, values out of range, and layout fields a node's parent
//! ignores.

use std::collections::BTreeMap;

use crate::model::{Form, Node, Widget};
use crate::schema::Catalog;

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
    pub fn validate(&self, catalog: &Catalog) -> Vec<Diagnostic> {
        let mut out = Vec::new();
        let mut names: BTreeMap<String, Names> = BTreeMap::new();
        if !self.name.is_empty() && !is_identifier(&self.name) {
            out.push(Diagnostic::new(
                Severity::Error,
                None,
                Some("name"),
                format!("`{}` is not a valid form name", self.name),
            ));
        }
        self.root.walk(&mut |node, parent| {
            check_place(node, parent, &mut out);
            check_layout(node, &mut out);
            if let Some(widget) = node.widget() {
                check_widget(widget, catalog, &mut names, &mut out);
            }
        });
        out
    }
}

/// How a name has been used so far.
#[derive(Default)]
struct Names {
    plain: bool,
    indices: Vec<u32>,
}

/// Checks a widget's name, control-array fields and property values.
fn check_widget(
    widget: &dyn Widget,
    catalog: &Catalog,
    names: &mut BTreeMap<String, Names>,
    out: &mut Vec<Diagnostic>,
) {
    let name = widget.name();
    let error = |property: &str, message: String| {
        Diagnostic::new(Severity::Error, Some(name), Some(property), message)
    };
    if name.is_empty() {
        if widget.array().is_some() || widget.index().is_some() {
            out.push(error("name", "a control array needs a name".to_owned()));
        }
    } else if !is_identifier(name) {
        out.push(error("name", format!("`{name}` is not a valid name")));
    } else {
        let used = names.entry(name.to_owned()).or_default();
        let indices: Vec<u32> = match (widget.array(), widget.index()) {
            (Some(_), Some(_)) => {
                out.push(error(
                    "array",
                    "a node sets either `array` or `index`, not both".to_owned(),
                ));
                Vec::new()
            }
            (Some(count), None) => (0..count).collect(),
            (None, Some(index)) => vec![index],
            (None, None) => Vec::new(),
        };
        let clash = if indices.is_empty() {
            used.plain || !used.indices.is_empty()
        } else {
            used.plain || indices.iter().any(|index| used.indices.contains(index))
        };
        if clash {
            out.push(error("name", format!("`{name}` is used more than once")));
        }
        used.plain |= indices.is_empty();
        used.indices.extend(indices);
    }
    let Some(spec) = catalog.get(widget.kind()) else {
        return;
    };
    for (property, value) in widget.props() {
        if let Some(spec) = spec.property(&property)
            && !spec.accepts(&value)
        {
            out.push(error(
                &property,
                format!("`{property}` must be {}", describe(&spec.ty)),
            ));
        }
    }
}

/// A property type in words, for a diagnostic.
fn describe(ty: &crate::value::ValueType) -> String {
    use crate::value::ValueType;
    match ty {
        ValueType::Int { min: Some(min), .. } => format!("an int of at least {min}"),
        other => format!("a valid {}", other.type_name()),
    }
}

/// Warns about layout fields the node's parent ignores.
fn check_place(node: &Node, parent: Option<&Node>, out: &mut Vec<Diagnostic>) {
    let place = node.place();
    let name = node.widget().map(Widget::name);
    let warn = |field: &str, message: &str| {
        Diagnostic::new(
            Severity::Warning,
            name,
            Some(field),
            format!("`{field}` {message}"),
        )
    };
    let in_absolute = matches!(parent, Some(Node::Absolute(_)));
    let in_grid = matches!(parent, Some(Node::Grid(_)));
    if !in_absolute {
        if place.at.is_some() {
            out.push(warn("at", "only places an entry of an `Absolute` layout"));
        }
        if place.anchor.is_some() {
            out.push(warn(
                "anchor",
                "only anchors an entry of an `Absolute` layout",
            ));
        }
    } else if place.at.is_none() {
        out.push(warn(
            "at",
            "is missing: the entry sits at its natural size in the corner",
        ));
    }
    if place.span.is_some() && !in_grid {
        out.push(warn("span", "only spans columns of a `Grid`"));
    }
}

/// Checks a layout's own fields.
fn check_layout(node: &Node, out: &mut Vec<Diagnostic>) {
    if let Node::Grid(grid) = node
        && grid.columns.is_empty()
    {
        out.push(Diagnostic::new(
            Severity::Error,
            None,
            Some("columns"),
            "a `Grid` needs at least one column".to_owned(),
        ));
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
