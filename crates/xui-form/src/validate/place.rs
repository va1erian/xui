#![forbid(unsafe_code)]

//! Checking where a node sits: layout fields its parent ignores, and a
//! layout's own fields.

use super::{Diagnostic, Severity};
use crate::model::{Node, Widget};

/// Warns about layout fields the node's parent ignores.
pub(super) fn check_place(node: &Node, parent: Option<&Node>, out: &mut Vec<Diagnostic>) {
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
pub(super) fn check_layout(node: &Node, out: &mut Vec<Diagnostic>) {
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
