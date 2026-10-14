#![forbid(unsafe_code)]

//! The scan that finds where a form exceeds a limit (see
//! [`limits`](super::limits)).
//!
//! `Form::validate` reports each breach as an error and `describe` refuses to
//! build a form that has one, whether or not it was validated: a form that
//! was edited in memory never went through the reader's checks. The scan does
//! not recurse and enumerates no array, so a tree of any depth, and an array
//! of any size, is scanned in time proportional to the file.

use super::limits::{MAX_ARRAY, MAX_DEPTH, MAX_LENGTH};
use super::{Form, Length, Node, Track, Widget};

/// One place a form exceeds a limit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Breach {
    /// The widget the breach is in, if it has a name.
    pub(crate) node: Option<String>,
    /// The field it is about.
    pub(crate) property: &'static str,
    /// What is wrong.
    pub(crate) message: String,
}

/// Whether `length` is one the format allows: finite and within
/// [`MAX_LENGTH`] of zero.
pub(crate) fn length_fits(length: Length) -> bool {
    length.0.is_finite() && length.0.abs() <= MAX_LENGTH
}

/// What is wrong with a length, as a sentence fragment.
pub(crate) fn length_message(value: f32) -> String {
    format!(
        "{value:?} is not a length: lengths are finite and within {MAX_LENGTH} design units of zero"
    )
}

/// Whether `node` is a container widget, whose content a control array would
/// rebuild for each element.
fn is_container(node: &Node) -> bool {
    matches!(node, Node::Panel(_) | Node::Group(_) | Node::Tabs(_))
}

/// Every place `form` exceeds a limit.
pub(crate) fn breaches(form: &Form) -> Vec<Breach> {
    let mut found = Vec::new();
    let depth = form.root.depth();
    if depth > MAX_DEPTH {
        found.push(Breach {
            node: None,
            property: "root",
            message: format!("the form nests {depth} levels deep; at most {MAX_DEPTH} are allowed"),
        });
    }
    for length in [form.size.0, form.size.1] {
        if !length_fits(length) {
            found.push(Breach {
                node: None,
                property: "size",
                message: format!("the window's size: {}", length_message(length.0)),
            });
        }
    }
    form.root.walk(&mut |node, _| {
        let name = node
            .widget()
            .map(Widget::name)
            .filter(|name| !name.is_empty())
            .map(str::to_owned);
        let mut breach = |property: &'static str, message: String| {
            found.push(Breach {
                node: name.clone(),
                property,
                message,
            });
        };
        for (property, length) in lengths(node) {
            if !length_fits(length) {
                breach(
                    property,
                    format!("`{property}`: {}", length_message(length.0)),
                );
            }
        }
        if let Some(widget) = node.widget() {
            array_breaches(node, widget, &mut breach);
        }
    });
    found
}

/// Every length `node` holds, with the field it is in.
fn lengths(node: &Node) -> Vec<(&'static str, Length)> {
    let place = node.place();
    let mut found = Vec::new();
    for (field, length) in [
        ("width", place.width),
        ("height", place.height),
        ("max_width", place.max_width),
        ("max_height", place.max_height),
    ] {
        found.extend(length.map(|length| (field, length)));
    }
    if let Some((x, y, width, height)) = place.at {
        found.extend([x, y, width, height].map(|length| ("at", length)));
    }
    match node {
        Node::Row(stack) | Node::Column(stack) | Node::Wrap(stack) => {
            found.extend([("gap", stack.gap), ("padding", stack.padding)]);
        }
        Node::Grid(grid) => {
            found.extend([("gap", grid.gap), ("padding", grid.padding)]);
            for track in &grid.columns {
                if let Track::Fixed(width) = track {
                    found.push(("columns", *width));
                }
            }
        }
        Node::Absolute(absolute) => {
            found.push(("padding", absolute.padding));
            if let Some((width, height)) = absolute.size {
                found.extend([width, height].map(|length| ("size", length)));
            }
        }
        _ => {}
    }
    found
}

/// The breaches of one widget's `array` and `index`.
fn array_breaches(node: &Node, widget: &dyn Widget, breach: &mut impl FnMut(&'static str, String)) {
    if let Some(count) = widget.array()
        && count > MAX_ARRAY
    {
        breach(
            "array",
            format!("`array` is {count}; a control array has at most {MAX_ARRAY} elements"),
        );
    }
    if let Some(index) = widget.index()
        && index >= MAX_ARRAY
    {
        breach(
            "index",
            format!("`index` is {index}; an element's index is below {MAX_ARRAY}"),
        );
    }
    if is_container(node) {
        for (property, set) in [
            ("array", widget.array().is_some()),
            ("index", widget.index().is_some()),
        ] {
            if set {
                breach(
                    property,
                    format!(
                        "a {} is a container, so `{property}` cannot repeat it: \
                         only widgets can be control arrays",
                        widget.kind()
                    ),
                );
            }
        }
    }
}
