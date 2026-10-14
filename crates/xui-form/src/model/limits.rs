#![forbid(unsafe_code)]

//! The limits of a form.
//!
//! A form is read from untrusted text and may be edited in memory, so what it
//! asks a build to make is bounded: its tree nests at most [`MAX_DEPTH`]
//! levels, a control array has at most [`MAX_ARRAY`] elements and a length is
//! within [`MAX_LENGTH`]. Each serves the reader (`load` rejects what exceeds
//! it, at its position) and the writer (`Form::to_ron` refuses it) alike, so a
//! form that is written can always be read back; the scan in
//! [`breaches`](super::breach) is what `Form::validate` reports and
//! `describe` refuses to build.

use std::cell::Cell;

use serde::de;

use super::Node;

/// The deepest a form's tree may nest: the root node is level 1, a node
/// directly inside it level 2, and so on. A `Group`'s content, a `Panel`'s
/// content and a `Tabs` page's content are each one level below the
/// container; content that is the default (omitted from a file) is no level.
///
/// A form nested more deeply is not read (`load` reports the first node one
/// level past this), not written (`Form::to_ron` returns an error), not built,
/// and `Form::validate` reports it.
///
/// The value keeps the reader inside a Windows main thread's 1 MiB of stack
/// in a debug build, the worst case: a debug build needs about 5 KiB for each
/// level of RON nesting, and a chain of `Tabs`, which nest eight per level,
/// takes about 775 KiB to load at 16 (a `Column` chain 583 KiB, a `Panel`
/// 519), and building adds 135 to 199 KiB at its own peak. A test runs every
/// kind of chain at this depth on a 1 MiB thread.
pub const MAX_DEPTH: usize = 16;

/// The most elements a control array has: `array` is at most this, and an
/// element's `index` is below it.
///
/// An array of `n` elements builds `n` widgets, so the count of a file would
/// otherwise be a way to exhaust memory and time (a `Button(array:
/// 100000000)` took 770 MB to check). 1024 is far beyond a form's real
/// controls; more rows belong in a `ListView`.
pub const MAX_ARRAY: u32 = 1024;

/// The largest length, in design units, either way: a width, a gap, a
/// position. A length is finite and at most this many units from zero.
///
/// Layout adds lengths in 32-bit device pixels, so a length the file may write
/// as `1e38` would overflow it (a panic in a debug build); 32768 is the range
/// classic window coordinates had and is far beyond any form.
pub const MAX_LENGTH: f32 = 32_768.0;

/// How many levels of RON nesting one level of node takes at most: the most
/// nested kind (a `Tabs` page: the node, its `pages` field, the list, the
/// `Page`, its `content` field, ...) sets it. A test measures every kind that
/// nests and fails when this is not the largest figure, so a new kind or a
/// change of serde's shape cannot quietly break the bound below.
pub(crate) const RON_LEVELS_PER_NODE: usize = 8;

/// The recursion limit the form's RON is read and written with: enough RON
/// levels for [`MAX_DEPTH`] nodes of the most nested kind, plus a few for the
/// form itself, so the limit never trips before [`MAX_DEPTH`] does: it only
/// stops text no valid form can be. It is also what keeps the reader's stack
/// within the bound that justifies `MAX_DEPTH` (see the 1 MiB test in
/// `tests/limits.rs`).
pub(crate) const RECURSION_LIMIT: usize = RON_LEVELS_PER_NODE * MAX_DEPTH + 8;

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
    ///
    /// The content of a `Panel`, a `Group` or a page that is the default (an
    /// empty column) is not written to a file and not read from one, so it is
    /// no level: the depth is that of the form the file spells out, which is
    /// what the reader and the writer limit.
    pub fn depth(&self) -> usize {
        let default = Node::default();
        let mut deepest = 0;
        let mut pending = vec![(self, 1usize)];
        while let Some((node, level)) = pending.pop() {
            deepest = deepest.max(level);
            // A container's content is omitted from a file when it is the
            // default; a layout's children are always spelt out.
            let (inside, content) = match node {
                Node::Panel(panel) => (vec![&*panel.content], true),
                Node::Group(group) => (vec![&*group.content], true),
                Node::Tabs(tabs) => (tabs.pages.iter().map(|page| &page.content).collect(), true),
                other => (other.children(), false),
            };
            pending.extend(
                inside
                    .into_iter()
                    .filter(|child| !content || **child != default)
                    .map(|child| (child, level + 1)),
            );
        }
        deepest
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codec::{options, pretty};
    use crate::model::{Absolute, Grid, Group, Label, Page, Panel, Stack, Tabs};
    use crate::schema::Catalog;
    use crate::{Form, load};

    /// Every kind of node that holds nodes.
    const NESTING: [&str; 8] = [
        "Row", "Column", "Wrap", "Grid", "Absolute", "Panel", "Group", "Tabs",
    ];

    /// `inner` inside a node of `kind`, in the field that holds nodes.
    fn wrap(kind: &str, inner: Node) -> Node {
        let stack = |inner| Stack {
            children: vec![inner],
            ..Stack::default()
        };
        match kind {
            "Row" => Node::Row(stack(inner)),
            "Column" => Node::Column(stack(inner)),
            "Wrap" => Node::Wrap(stack(inner)),
            "Grid" => Node::Grid(Grid {
                children: vec![inner],
                ..Grid::default()
            }),
            "Absolute" => Node::Absolute(Absolute {
                children: vec![inner],
                ..Absolute::default()
            }),
            "Panel" => Node::Panel(Panel {
                content: Box::new(inner),
                ..Panel::default()
            }),
            "Group" => Node::Group(Group {
                content: Box::new(inner),
                ..Group::default()
            }),
            "Tabs" => Node::Tabs(Tabs {
                pages: vec![Page {
                    title: "t".to_owned(),
                    content: inner,
                }],
                ..Tabs::default()
            }),
            other => panic!("`{other}` is not in NESTING"),
        }
    }

    /// Whether a node of this variant holds nodes. It names every variant
    /// and has no catch-all: a new kind of node does not compile until it is
    /// classified here, and a nesting one is then missing from `NESTING`.
    fn nests(node: &Node) -> bool {
        match node {
            Node::Row(_)
            | Node::Column(_)
            | Node::Wrap(_)
            | Node::Grid(_)
            | Node::Absolute(_)
            | Node::Panel(_)
            | Node::Group(_)
            | Node::Tabs(_) => true,
            Node::Label(_)
            | Node::Button(_)
            | Node::Hyperlink(_)
            | Node::CheckBox(_)
            | Node::ToggleButton(_)
            | Node::Edit(_)
            | Node::MultilineEdit(_)
            | Node::NumberField(_)
            | Node::Slider(_)
            | Node::ProgressBar(_)
            | Node::RadioGroup(_)
            | Node::ComboBox(_)
            | Node::ListView(_)
            | Node::Separator(_) => false,
        }
    }

    /// `depth` levels of `kind`, ending in a label.
    fn chain(kind: &str, depth: usize) -> Form {
        let mut node = Node::Label(Label::default());
        for _ in 1..depth {
            node = wrap(kind, node);
        }
        Form {
            root: node,
            ..Form::default()
        }
    }

    /// The least recursion limit that reads `text`.
    fn levels_to_read(text: &str) -> usize {
        (1..=RECURSION_LIMIT + 16)
            .find(|&limit| {
                options()
                    .with_recursion_limit(limit)
                    .from_str::<Form>(text)
                    .is_ok()
            })
            .expect("a chain at the limit reads with some limit")
    }

    /// The least recursion limit that writes `form`.
    fn levels_to_write(form: &Form) -> usize {
        (1..=RECURSION_LIMIT + 16)
            .find(|&limit| {
                options()
                    .with_recursion_limit(limit)
                    .to_string_pretty(form, pretty())
                    .is_ok()
            })
            .expect("a chain at the limit writes with some limit")
    }

    #[test]
    fn every_kind_that_nests_is_listed() {
        let catalog = Catalog::xui();
        let kinds = catalog
            .kinds()
            .map(str::to_owned)
            .chain(catalog.layouts().iter().map(|layout| layout.kind.clone()));
        for kind in kinds {
            let node = load(&format!("Form(root: {kind}())"))
                .unwrap_or_else(|error| panic!("{kind}: {error}"))
                .root;
            assert_eq!(nests(&node), NESTING.contains(&kind.as_str()), "{kind}");
        }
        for kind in NESTING {
            let node = wrap(kind, Node::Label(Label::default()));
            assert_eq!(node.kind(), kind);
            assert_eq!(node.children().len(), 1, "{kind} has one place for nodes");
        }
    }

    #[test]
    fn the_ron_levels_per_node_are_the_most_any_kind_takes() {
        let mut most = 0;
        for kind in NESTING {
            let read = |depth| {
                let text = chain(kind, depth).to_ron().expect("it is written");
                levels_to_read(&text)
            };
            let write = |depth| levels_to_write(&chain(kind, depth));
            let per_node = (read(MAX_DEPTH) - read(MAX_DEPTH - 1))
                .max(write(MAX_DEPTH) - write(MAX_DEPTH - 1));
            assert!(
                per_node <= RON_LEVELS_PER_NODE,
                "{kind} takes {per_node} RON levels a node"
            );
            most = most.max(per_node);
            // At the limit, the reader and the writer are within the limit.
            let text = chain(kind, MAX_DEPTH).to_ron().expect("it is written");
            assert!(levels_to_read(&text) <= RECURSION_LIMIT, "{kind}");
            assert!(
                levels_to_write(&chain(kind, MAX_DEPTH)) <= RECURSION_LIMIT,
                "{kind}"
            );
        }
        assert_eq!(
            most, RON_LEVELS_PER_NODE,
            "the constant is the measured figure"
        );
    }
}
