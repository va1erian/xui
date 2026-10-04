#![forbid(unsafe_code)]

//! A pure layout tree: nested rows, columns and grids of keyed leaves.
//!
//! The tree knows nothing about widgets. A leaf is an opaque key `K`; what the
//! layout needs to know about it (its natural size within some
//! [`Constraints`], whether it is visible, and the insets of any content it
//! frames) is asked for through a [`Leaf`] callback each time the tree is laid
//! out, so a text change or a hidden widget is picked up without rebuilding
//! the tree. Slot arithmetic is the existing [`Stack`](super::Stack), so
//! leftover pixels are shared with largest-remainder rounding and there are no
//! gaps or overlaps.

mod flow;
mod grid;
#[cfg(test)]
mod tests;
mod types;

pub use types::{Align, Constraints, Leaf, LeafFn, Sizing, Track};

use super::{Insets, StackDirection};
use crate::geometry::{Rect, Size};
use crate::units::Dip;

/// One entry of a [`Group`]: a leaf, a nested group, or a leaf that frames a
/// nested group, with its sizing and placement.
#[derive(Clone, Debug)]
pub struct Item<K> {
    content: Content<K>,
    sizing: Sizing,
    align: Option<Align>,
    max_width: Option<Dip>,
    max_height: Option<Dip>,
    span: usize,
}

#[derive(Clone, Debug)]
enum Content<K> {
    Leaf(K),
    Group(Group<K>),
    Framed(K, Group<K>),
}

impl<K: Copy> Item<K> {
    fn new(content: Content<K>) -> Item<K> {
        Item {
            content,
            sizing: Sizing::Auto,
            align: None,
            max_width: None,
            max_height: None,
            span: 1,
        }
    }

    /// A leaf keyed by `key`, at its natural size.
    pub fn leaf(key: K) -> Item<K> {
        Item::new(Content::Leaf(key))
    }

    /// A nested group, at its natural size.
    pub fn group(group: Group<K>) -> Item<K> {
        Item::new(Content::Group(group))
    }

    /// The leaf `key` with `group` laid out inside it, within the insets the
    /// leaf reports as [`Leaf::content`].
    pub fn framed(key: K, group: Group<K>) -> Item<K> {
        Item::new(Content::Framed(key, group))
    }

    /// Sets how the item is sized.
    pub fn sized(mut self, sizing: Sizing) -> Item<K> {
        self.sizing = sizing;
        self
    }

    /// The item's sizing.
    pub fn sizing(&self) -> Sizing {
        self.sizing
    }

    /// Places the item across its parent's cross axis (in a grid, within its
    /// cell on both axes) instead of the parent's default.
    pub fn align(mut self, align: Align) -> Item<K> {
        self.align = Some(align);
        self
    }

    /// Caps the item's width at `width` design units.
    pub fn max_width(mut self, width: Dip) -> Item<K> {
        self.max_width = Some(width);
        self
    }

    /// Caps the item's height at `height` design units.
    pub fn max_height(mut self, height: Dip) -> Item<K> {
        self.max_height = Some(height);
        self
    }

    /// In a grid, makes the item cover `columns` columns (at least one).
    pub fn span(mut self, columns: usize) -> Item<K> {
        self.span = columns.max(1);
        self
    }

    fn is_visible(&self, leaf: LeafFn<'_, K>, dpi: u32) -> bool {
        match &self.content {
            Content::Leaf(key) | Content::Framed(key, _) => {
                leaf(key, Constraints::unbounded(dpi)).visible
            }
            Content::Group(group) => group.items.iter().any(|item| item.is_visible(leaf, dpi)),
        }
    }

    /// The content's natural size within `constraints`, before the item's
    /// own sizing applies.
    fn measure(&self, constraints: Constraints, leaf: LeafFn<'_, K>) -> Size {
        match &self.content {
            Content::Leaf(key) => leaf(key, constraints).natural,
            Content::Group(group) => group.measure(constraints, leaf),
            Content::Framed(key, group) => {
                let frame = leaf(key, constraints);
                let inner = group.measure(constraints.shrink(frame.content), leaf);
                let px = |value: Dip| value.to_px(constraints.dpi).value();
                let insets = frame.content;
                Size::new(
                    frame
                        .natural
                        .width
                        .max(inner.width + px(insets.left) + px(insets.right)),
                    frame
                        .natural
                        .height
                        .max(inner.height + px(insets.top) + px(insets.bottom)),
                )
            }
        }
    }

    /// The item's natural size within `constraints` as its parent lays it
    /// along `direction`: the content's size with the sizing and the caps
    /// applied. A fill item has no natural main extent of its own.
    fn natural(
        &self,
        direction: StackDirection,
        constraints: Constraints,
        leaf: LeafFn<'_, K>,
    ) -> Size {
        let dpi = constraints.dpi;
        let px = |value: Dip| value.to_px(dpi).value().max(0);
        let content = self.measure(constraints, leaf);
        let sized = match self.sizing {
            Sizing::Width(value) => Size::new(px(value), content.height),
            Sizing::Height(value) => Size::new(content.width, px(value)),
            sizing => {
                let main_extent = match sizing {
                    Sizing::Fixed(value) => px(value),
                    Sizing::Min(value) => main(direction, content).max(px(value)),
                    Sizing::Fill(_) => 0,
                    _ => main(direction, content),
                };
                with_main(direction, content, main_extent)
            }
        };
        self.capped(sized, dpi)
    }

    /// `size` with the item's caps applied.
    fn capped(&self, size: Size, dpi: u32) -> Size {
        let cap = |value: i32, max: Option<Dip>| match max {
            Some(max) => value.min(max.to_px(dpi).value().max(0)),
            None => value,
        };
        Size::new(
            cap(size.width, self.max_width),
            cap(size.height, self.max_height),
        )
    }

    fn place(&self, rect: Rect, dpi: u32, leaf: LeafFn<'_, K>, out: &mut Vec<(K, Rect)>) {
        match &self.content {
            Content::Leaf(key) => out.push((*key, rect)),
            Content::Group(group) => group.place(rect, dpi, leaf, out),
            Content::Framed(key, group) => {
                out.push((*key, rect));
                let insets = leaf(key, Constraints::unbounded(dpi)).content;
                group.place(insets.apply(rect, dpi), dpi, leaf, out);
            }
        }
    }
}

/// How a [`Group`] arranges its items.
#[derive(Clone, Debug)]
enum Arrangement {
    Stack(StackDirection),
    Grid(Vec<Track>),
}

/// A row, column or grid of [`Item`]s.
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

    /// The gap between adjacent items (in a grid, between rows and between
    /// columns), in design units.
    pub fn spacing(mut self, spacing: Dip) -> Group<K> {
        self.spacing = spacing;
        self
    }

    /// Margins inside the parent, in design units.
    pub fn margins(mut self, margins: Insets) -> Group<K> {
        self.margins = margins;
        self
    }

    /// Where items sit across the main axis (in a grid, within their cells)
    /// unless they set their own [`Item::align`]. The default stretches them.
    pub fn align(mut self, align: Align) -> Group<K> {
        self.align = align;
        self
    }

    /// Where a row's or column's items sit along the main axis when none of
    /// them fills it. The default packs them at the start; `Stretch` is the
    /// same as `Start`. A grid ignores it.
    pub fn justify(mut self, justify: Align) -> Group<K> {
        self.justify = justify;
        self
    }

    /// Appends an item.
    pub fn push(mut self, item: Item<K>) -> Group<K> {
        self.items.push(item);
        self
    }

    /// Lays the tree out inside `rect`, returning one rectangle per visible
    /// leaf, in tree order.
    pub fn compute(&self, rect: Rect, dpi: u32, leaf: LeafFn<'_, K>) -> Vec<(K, Rect)> {
        let mut out = Vec::new();
        self.place(rect, dpi, leaf, &mut out);
        out
    }

    fn place(&self, rect: Rect, dpi: u32, leaf: LeafFn<'_, K>, out: &mut Vec<(K, Rect)>) {
        match &self.arrangement {
            Arrangement::Stack(direction) => flow::place(self, *direction, rect, dpi, leaf, out),
            Arrangement::Grid(columns) => grid::place(self, columns, rect, dpi, leaf, out),
        }
    }

    /// The size the group's content wants, in device pixels at `dpi`, with no
    /// bound on either axis. A [`Sizing::Fill`] item contributes no natural
    /// extent along its parent's main axis.
    pub fn preferred_size(&self, dpi: u32, leaf: LeafFn<'_, K>) -> Size {
        self.measure(Constraints::unbounded(dpi), leaf)
    }

    /// The size the group's content wants within `constraints`.
    fn measure(&self, constraints: Constraints, leaf: LeafFn<'_, K>) -> Size {
        match &self.arrangement {
            Arrangement::Stack(direction) => flow::measure(self, *direction, constraints, leaf),
            Arrangement::Grid(columns) => grid::measure(self, columns, constraints, leaf),
        }
    }

    fn visible_items(&self, leaf: LeafFn<'_, K>, dpi: u32) -> Vec<&Item<K>> {
        self.items
            .iter()
            .filter(|item| item.is_visible(leaf, dpi))
            .collect()
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
