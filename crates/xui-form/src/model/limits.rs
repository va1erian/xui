#![forbid(unsafe_code)]

//! The limits of a form, and the scan that reports where one exceeds them.
//!
//! A form is read from untrusted text and may be edited in memory, so what it
//! asks a build to make is bounded: its tree nests at most [`MAX_DEPTH`]
//! levels, and a control array has at most [`MAX_ARRAY`] elements. The depth
//! limit serves the reader (`load` rejects a deeper node at its position) and
//! the writer (`Form::to_ron` refuses a deeper tree) alike, so a form that is
//! written can always be read back; `Form::validate` reports every breach and
//! `describe` refuses to build one.

use std::cell::Cell;

use serde::de;

use super::{Form, Node, Widget};

/// The deepest a form's tree may nest: the root node is level 1, a node
/// directly inside it level 2, and so on. A `Group`'s content, a `Panel`'s
/// content and a `Tabs` page's content are each one level below the
/// container.
///
/// A form nested more deeply is not read (`load` reports the first node at
/// level 17), not written (`Form::to_ron` returns an error), not built, and
/// `Form::validate` reports it.
///
/// The value keeps the reader's stack use at what RON's default recursion
/// limit allowed (a debug build needs about 5 KiB of stack for each level of
/// RON nesting, and a `Tabs` chain nests eight per level).
pub const MAX_DEPTH: usize = 16;

/// The most elements a control array has: `array` is at most this, and an
/// element's `index` is below it.
///
/// An array of `n` elements builds `n` widgets, so the count of a file would
/// otherwise be a way to exhaust memory and time (a `Button(array:
/// 100000000)` took 770 MB to check). 1024 is far beyond a form's real
/// controls; more rows belong in a `ListView`.
pub const MAX_ARRAY: u32 = 1024;

/// The recursion limit the form's RON is read and written with. Eight is how
/// many RON levels the most nested node kind (a `Tabs` page) takes, plus a few
/// for the form itself, so the limit never trips before [`MAX_DEPTH`] does:
/// it only stops text no valid form can be.
pub(crate) const RECURSION_LIMIT: usize = 8 * MAX_DEPTH + 8;

/// What the reader says of a node past [`MAX_DEPTH`].
pub(crate) fn depth_message() -> String {
    format!("a form nests at most {MAX_DEPTH} levels deep")
}

thread_local! {
    /// How many nodes the reader on this thread is inside.
    static READING: Cell<usize> = const { Cell::new(0) };
}

/// One node being read: it counts towards [`MAX_DEPTH`] until dropped.
pub(super) struct Level;

impl Level {
    /// Enters one level, or fails with the position-less error the reader
    /// attaches its position to.
    pub(super) fn enter<E: de::Error>() -> Result<Level, E> {
        READING.with(|reading| {
            if reading.get() >= MAX_DEPTH {
                Err(E::custom(depth_message()))
            } else {
                reading.set(reading.get() + 1);
                Ok(Level)
            }
        })
    }
}

impl Drop for Level {
    fn drop(&mut self) {
        READING.with(|reading| reading.set(reading.get().saturating_sub(1)));
    }
}

impl Node {
    /// How deep the tree nests from this node: 1 for a node with nothing
    /// inside it. Walks the tree without recursing, so a tree of any depth is
    /// measured.
    pub fn depth(&self) -> usize {
        let mut deepest = 0;
        let mut pending = vec![(self, 1usize)];
        while let Some((node, level)) = pending.pop() {
            deepest = deepest.max(level);
            pending.extend(node.children().into_iter().map(|child| (child, level + 1)));
        }
        deepest
    }
}

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

/// Whether `node` is a container widget, whose content a control array would
/// rebuild for each element.
fn is_container(node: &Node) -> bool {
    matches!(node, Node::Panel(_) | Node::Group(_) | Node::Tabs(_))
}

/// Every place `form` exceeds a limit. Neither `validate` nor `describe`
/// recurses to find them, so a tree of any depth is scanned, and no array is
/// enumerated.
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
    form.root.walk(&mut |node, _| {
        if let Some(widget) = node.widget() {
            widget_breaches(node, widget, &mut found);
        }
    });
    found
}

/// The breaches of one widget's `array` and `index`.
fn widget_breaches(node: &Node, widget: &dyn Widget, found: &mut Vec<Breach>) {
    let name = Some(widget.name().to_owned()).filter(|name| !name.is_empty());
    let mut breach = |property: &'static str, message: String| {
        found.push(Breach {
            node: name.clone(),
            property,
            message,
        });
    };
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
