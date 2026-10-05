#![forbid(unsafe_code)]

//! A pure layout tree: nested rows, columns, grids, wraps, layers and
//! absolute groups of keyed leaves.
//!
//! The tree knows nothing about widgets. A leaf is an opaque key `K`; what the
//! layout needs to know about it (its natural size within some
//! [`Constraints`], whether it is visible, and the insets of any content it
//! frames) is asked for through a [`Leaf`] callback each time the tree is laid
//! out, so a text change or a hidden widget is picked up without rebuilding
//! the tree. Slot arithmetic is the existing [`Stack`](super::Stack), so
//! leftover pixels are shared with largest-remainder rounding and there are no
//! gaps or overlaps.

mod absolute;
mod flow;
mod grid;
mod item;
mod layered;
#[cfg(test)]
mod tests;
mod trace;
mod types;
mod wrap;

pub use item::Item;
pub use trace::{GroupKind, TraceNode, Traced, Warning};
pub use types::{Align, Constraints, Leaf, LeafFn, Sizing, Track};

use super::{Insets, StackDirection};
use crate::geometry::{Rect, Size};
use crate::units::Dip;

/// How a [`Group`] arranges its items.
#[derive(Clone, Debug)]
enum Arrangement {
    Stack(StackDirection),
    Grid(Vec<Track>),
    Wrap,
    Layered,
    /// Free placement, with the size the positions were designed at.
    Absolute(Option<(Dip, Dip)>),
}

/// A row, column, grid, wrap, layered or absolute group of [`Item`]s.
#[derive(Clone, Debug)]
pub struct Group<K> {
    arrangement: Arrangement,
    spacing: Dip,
    margins: Insets,
    align: Align,
    justify: Align,
    items: Vec<Item<K>>,
}

impl<K: Copy> Group<K> {
    /// A group that places its items left to right.
    pub const fn row() -> Group<K> {
        Group::new(Arrangement::Stack(StackDirection::Horizontal))
    }

    /// A group that places its items top to bottom.
    pub const fn column() -> Group<K> {
        Group::new(Arrangement::Stack(StackDirection::Vertical))
    }

    /// A group that places its items in `columns`, left to right and then
    /// row by row; an item spanning more columns than remain in a row starts
    /// the next one.
    pub fn grid(columns: Vec<Track>) -> Group<K> {
        let columns = if columns.is_empty() {
            vec![Track::Fill(1)]
        } else {
            columns
        };
        Group::new(Arrangement::Grid(columns))
    }

    /// A group that places its items left to right at their natural sizes,
    /// starting a new line when the next one does not fit: toolbars, chips,
    /// tiles. A line is as tall as its tallest item.
    pub const fn wrap() -> Group<K> {
        Group::new(Arrangement::Wrap)
    }

    /// A group that layers its items over one another in its whole area, the
    /// later above the earlier. Each item sits where its alignment puts it on
    /// both axes.
    pub const fn layered() -> Group<K> {
        Group::new(Arrangement::Layered)
    }

    /// A group that places each item where [`Item::at`] says, following the
    /// group's size through the item's [`Item::anchor`]. Items without a
    /// position sit at the top-left corner at their natural size.
    pub const fn absolute() -> Group<K> {
        Group::new(Arrangement::Absolute(None))
    }

    const fn new(arrangement: Arrangement) -> Group<K> {
        Group {
            arrangement,
            spacing: Dip(0.0),
            margins: Insets::new(Dip(0.0), Dip(0.0), Dip(0.0), Dip(0.0)),
            align: Align::Stretch,
            justify: Align::Start,
            items: Vec::new(),
        }
    }

    /// The gap between adjacent items (in a grid or a wrap, between rows and
    /// between columns), in design units.
    pub fn spacing(mut self, spacing: Dip) -> Group<K> {
        self.spacing = spacing;
        self
    }

    /// Margins inside the parent, in design units.
    pub fn margins(mut self, margins: Insets) -> Group<K> {
        self.margins = margins;
        self
    }

    /// Where items sit across the main axis (in a grid or a layered group,
    /// within their area on both axes; in a wrap, within their line) unless
    /// they set their own [`Item::align`]. The default stretches them.
    pub fn align(mut self, align: Align) -> Group<K> {
        self.align = align;
        self
    }

    /// Where a row's, column's or wrap line's items sit along the main axis
    /// when none of them fills it. The default packs them at the start;
    /// `Stretch` is the same as `Start`. Other groups ignore it.
    pub fn justify(mut self, justify: Align) -> Group<K> {
        self.justify = justify;
        self
    }

    /// For an absolute group, the inner size its positions were designed at:
    /// the anchors move and stretch items by the difference between this and
    /// the size the group is laid out at. Without it, the design size is the
    /// smallest that holds every positioned item. Other groups ignore it.
    pub fn design_size(mut self, width: Dip, height: Dip) -> Group<K> {
        if let Arrangement::Absolute(size) = &mut self.arrangement {
            *size = Some((width, height));
        }
        self
    }

    /// Appends an item.
    pub fn push(mut self, item: Item<K>) -> Group<K> {
        self.items.push(item);
        self
    }

    /// What kind of group this is.
    pub fn kind(&self) -> GroupKind {
        match self.arrangement {
            Arrangement::Stack(StackDirection::Horizontal) => GroupKind::Row,
            Arrangement::Stack(StackDirection::Vertical) => GroupKind::Column,
            Arrangement::Grid(_) => GroupKind::Grid,
            Arrangement::Wrap => GroupKind::Wrap,
            Arrangement::Layered => GroupKind::Layered,
            Arrangement::Absolute(_) => GroupKind::Absolute,
        }
    }

    /// Lays the tree out inside `rect`, returning one rectangle per visible
    /// leaf, in tree order.
    pub fn compute(&self, rect: Rect, dpi: u32, leaf: LeafFn<'_, K>) -> Vec<(K, Rect)> {
        let mut out = Out::new(false);
        self.place(rect, dpi, leaf, &mut out);
        out.leaves
    }

    fn place(&self, rect: Rect, dpi: u32, leaf: LeafFn<'_, K>, out: &mut Out<K>) {
        out.group(self.kind(), rect);
        out.nested(|out| match &self.arrangement {
            Arrangement::Stack(direction) => flow::place(self, *direction, rect, dpi, leaf, out),
            Arrangement::Grid(columns) => grid::place(self, columns, rect, dpi, leaf, out),
            Arrangement::Wrap => wrap::place(self, rect, dpi, leaf, out),
            Arrangement::Layered => layered::place(self, rect, dpi, leaf, out),
            Arrangement::Absolute(design) => absolute::place(self, *design, rect, dpi, leaf, out),
        });
    }

    /// The size the group's content wants, in device pixels at `dpi`, with no
    /// bound on either axis. A [`Sizing::Fill`] item contributes no natural
    /// extent along its parent's main axis.
    pub fn preferred_size(&self, dpi: u32, leaf: LeafFn<'_, K>) -> Size {
        self.measure(Constraints::unbounded(dpi), leaf)
    }

    /// The size the group's content wants within `constraints`: with a width
    /// bound, a wrap breaks its lines and a column measures wrapping leaves at
    /// that width, so the height is the one the group needs at that width.
    pub fn measure(&self, constraints: Constraints, leaf: LeafFn<'_, K>) -> Size {
        match &self.arrangement {
            Arrangement::Stack(direction) => flow::measure(self, *direction, constraints, leaf),
            Arrangement::Grid(columns) => grid::measure(self, columns, constraints, leaf),
            Arrangement::Wrap => wrap::measure(self, constraints, leaf),
            Arrangement::Layered => layered::measure(self, constraints, leaf),
            Arrangement::Absolute(design) => absolute::measure(self, *design, constraints, leaf),
        }
    }

    fn visible_items(&self, leaf: LeafFn<'_, K>, dpi: u32) -> Vec<&Item<K>> {
        self.items
            .iter()
            .filter(|item| item.is_visible(leaf, dpi))
            .collect()
    }

    /// The device-pixel margins at `dpi`: left plus right, top plus bottom.
    fn margin_size(&self, dpi: u32) -> Size {
        let px = |value: Dip| value.to_px(dpi).value().max(0);
        let m = self.margins;
        Size::new(px(m.left) + px(m.right), px(m.top) + px(m.bottom))
    }
}

/// What a layout pass produces: the leaves' rectangles and, for a
/// [`Group::trace`], every node with its depth.
pub(super) struct Out<K> {
    leaves: Vec<(K, Rect)>,
    trace: Option<Vec<Traced<K>>>,
    depth: usize,
}

impl<K: Copy> Out<K> {
    fn new(trace: bool) -> Out<K> {
        Out {
            leaves: Vec::new(),
            trace: trace.then(Vec::new),
            depth: 0,
        }
    }

    /// Records a leaf placed at `rect`.
    fn leaf(&mut self, key: K, rect: Rect) {
        self.leaves.push((key, rect));
        self.record(TraceNode::Leaf(key), rect);
    }

    /// Records a group laid out in `rect`.
    fn group(&mut self, kind: GroupKind, rect: Rect) {
        self.record(TraceNode::Group(kind), rect);
    }

    fn record(&mut self, node: TraceNode<K>, rect: Rect) {
        if let Some(trace) = &mut self.trace {
            trace.push(Traced {
                depth: self.depth,
                node,
                rect,
                warnings: Vec::new(),
            });
        }
    }

    /// Runs `f` one level deeper, for the content of the node just recorded.
    fn nested(&mut self, f: impl FnOnce(&mut Out<K>)) {
        self.depth += 1;
        f(self);
        self.depth -= 1;
    }
}

/// The extent of `size` along `direction`.
fn main(direction: StackDirection, size: Size) -> i32 {
    match direction {
        StackDirection::Horizontal => size.width,
        StackDirection::Vertical => size.height,
    }
}

/// The extent of `size` across `direction`.
fn cross(direction: StackDirection, size: Size) -> i32 {
    match direction {
        StackDirection::Horizontal => size.height,
        StackDirection::Vertical => size.width,
    }
}

/// `size` with its extent along `direction` replaced by `extent`.
fn with_main(direction: StackDirection, size: Size, extent: i32) -> Size {
    match direction {
        StackDirection::Horizontal => Size::new(extent, size.height),
        StackDirection::Vertical => Size::new(size.width, extent),
    }
}

/// Places a span of `extent` pixels inside `start..end` by `align`. `Stretch`
/// fills the span.
fn align_span(start: i32, end: i32, extent: i32, align: Align) -> (i32, i32) {
    let room = (end - start).max(0);
    let extent = extent.clamp(0, room);
    match align {
        Align::Stretch => (start, end),
        Align::Start => (start, start + extent),
        Align::Center => {
            let from = start + (room - extent) / 2;
            (from, from + extent)
        }
        Align::End => (end - extent, end),
    }
}

/// Narrows `area` on both axes to `size`, placed by `align`; `Stretch` keeps
/// the whole area.
fn align_both(area: Rect, size: Size, align: Align) -> Rect {
    if align == Align::Stretch {
        return area;
    }
    let (left, right) = align_span(area.left, area.right, size.width, align);
    let (top, bottom) = align_span(area.top, area.bottom, size.height, align);
    Rect::new(left, top, right, bottom)
}

/// `rect` with each axis narrowed to the item's caps, kept where `align`
/// says (a stretched item keeps its start edge).
fn clamp_to_caps<K: Copy>(item: &Item<K>, rect: Rect, align: Align, dpi: u32) -> Rect {
    let align = if align == Align::Stretch {
        Align::Start
    } else {
        align
    };
    let mut rect = rect;
    if let Some(max) = item.max_width {
        let max = max.to_px(dpi).value().max(0);
        if rect.width() > max {
            let (left, right) = align_span(rect.left, rect.right, max, align);
            rect = Rect::new(left, rect.top, right, rect.bottom);
        }
    }
    if let Some(max) = item.max_height {
        let max = max.to_px(dpi).value().max(0);
        if rect.height() > max {
            let (top, bottom) = align_span(rect.top, rect.bottom, max, align);
            rect = Rect::new(rect.left, top, rect.right, bottom);
        }
    }
    rect
}
