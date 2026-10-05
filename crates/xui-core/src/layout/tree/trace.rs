#![forbid(unsafe_code)]

//! [`Group::trace`]: every node of a laid-out tree with its rectangle, and the
//! problems a reader (often an agent) should look at.

use super::{Group, LeafFn, Out};
use crate::geometry::Rect;

/// What kind of [`Group`] a traced node is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GroupKind {
    /// Items left to right.
    Row,
    /// Items top to bottom.
    Column,
    /// Items in columns, row by row.
    Grid,
    /// Items in lines that wrap.
    Wrap,
    /// Items layered over one another.
    Layered,
    /// Items at free positions.
    Absolute,
}

impl GroupKind {
    /// Whether the group's items must not overlap: true for rows, columns,
    /// grids and wraps; layered and absolute groups overlap on purpose.
    pub const fn disjoint(self) -> bool {
        matches!(
            self,
            GroupKind::Row | GroupKind::Column | GroupKind::Grid | GroupKind::Wrap
        )
    }

    /// The group's name as the builders spell it.
    pub const fn name(self) -> &'static str {
        match self {
            GroupKind::Row => "row",
            GroupKind::Column => "column",
            GroupKind::Grid => "grid",
            GroupKind::Wrap => "wrap",
            GroupKind::Layered => "layered",
            GroupKind::Absolute => "absolute",
        }
    }
}

/// A traced node: a group or a leaf.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TraceNode<K> {
    /// A group of the given kind.
    Group(GroupKind),
    /// The leaf with this key.
    Leaf(K),
}

/// A problem with a traced node.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Warning {
    /// The node reaches outside the area its parent gave it, so part of it
    /// is clipped.
    Outside,
    /// The node is visible but has no width or no height.
    ZeroSize,
    /// The node overlaps the sibling at this index of the trace, inside a
    /// group whose items must be disjoint.
    Overlaps(usize),
}

/// One node of a [`Group::trace`], in tree order.
#[derive(Clone, Debug, PartialEq)]
pub struct Traced<K> {
    /// How deep the node is: the root group is 0, its items 1, and a framed
    /// leaf's content one deeper than the leaf.
    pub depth: usize,
    /// The node.
    pub node: TraceNode<K>,
    /// Where it was placed, in device pixels.
    pub rect: Rect,
    /// What looks wrong with it.
    pub warnings: Vec<Warning>,
}

impl<K: Copy> Group<K> {
    /// Lays the tree out inside `rect` like [`compute`](Self::compute) and
    /// returns every visible node (groups included) with its rectangle and
    /// warnings: a node outside its parent, a visible leaf of zero size, and
    /// siblings that overlap inside a [disjoint](GroupKind::disjoint) group.
    pub fn trace(&self, rect: Rect, dpi: u32, leaf: LeafFn<'_, K>) -> Vec<Traced<K>> {
        let mut out = Out::new(true);
        self.place(rect, dpi, leaf, &mut out);
        let mut trace = out.trace.unwrap_or_default();
        warn(&mut trace, rect);
        trace
    }
}

/// Fills in each node's warnings; `area` is the root's parent area.
fn warn<K>(trace: &mut [Traced<K>], area: Rect) {
    for index in 0..trace.len() {
        let depth = trace[index].depth;
        let parent = trace[..index]
            .iter()
            .rev()
            .find(|node| node.depth + 1 == depth)
            .map_or(area, |node| node.rect);
        let rect = trace[index].rect;
        if !inside(rect, parent) {
            trace[index].warnings.push(Warning::Outside);
        }
        if matches!(trace[index].node, TraceNode::Leaf(_))
            && (rect.width() <= 0 || rect.height() <= 0)
        {
            trace[index].warnings.push(Warning::ZeroSize);
        }
        let TraceNode::Group(kind) = trace[index].node else {
            continue;
        };
        if !kind.disjoint() {
            continue;
        }
        let children: Vec<usize> = (index + 1..trace.len())
            .take_while(|&child| trace[child].depth > depth)
            .filter(|&child| trace[child].depth == depth + 1)
            .collect();
        for (n, &a) in children.iter().enumerate() {
            for &b in &children[n + 1..] {
                if overlap(trace[a].rect, trace[b].rect) {
                    trace[a].warnings.push(Warning::Overlaps(b));
                    trace[b].warnings.push(Warning::Overlaps(a));
                }
            }
        }
    }
}

/// Whether `rect` lies within `area`.
fn inside(rect: Rect, area: Rect) -> bool {
    rect.left >= area.left
        && rect.top >= area.top
        && rect.right <= area.right
        && rect.bottom <= area.bottom
}

/// Whether two rectangles share some area.
fn overlap(a: Rect, b: Rect) -> bool {
    a.left.max(b.left) < a.right.min(b.right) && a.top.max(b.top) < a.bottom.min(b.bottom)
}
