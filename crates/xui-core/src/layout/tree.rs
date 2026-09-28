#![forbid(unsafe_code)]

//! A pure layout tree: nested rows and columns of keyed leaves.
//!
//! The tree knows nothing about widgets. A leaf is an opaque key `K`; what the
//! layout needs to know about it (its natural size and whether it is visible)
//! is asked for through a [`Leaf`] callback each time the tree is laid out, so
//! a text change or a hidden widget is picked up without rebuilding the tree.
//! Slot arithmetic is the existing [`Stack`], so leftover pixels are shared with
//! largest-remainder rounding and there are no gaps or overlaps.

use super::{Insets, Stack, StackDirection, StackSlot};
use crate::geometry::{Rect, Size};
use crate::units::{Dip, Px};

/// What the tree asks about a leaf when it lays out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Leaf {
    /// The leaf's natural size, in device pixels.
    pub natural: Size,
    /// Whether the leaf takes part in layout. A hidden leaf takes no space.
    pub visible: bool,
}

impl Leaf {
    /// A visible leaf of `natural` size.
    pub const fn new(natural: Size) -> Leaf {
        Leaf {
            natural,
            visible: true,
        }
    }
}

/// How an [`Item`] is sized: along its parent's main axis, or, for
/// [`Width`](Sizing::Width) and [`Height`](Sizing::Height), along a named axis.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Sizing {
    /// The natural size along the main axis.
    Auto,
    /// Exactly this many design units along the main axis.
    Fixed(Dip),
    /// At least this many design units along the main axis; shrinks only when
    /// the parent is too small to honour every item.
    Min(Dip),
    /// A share of the leftover main-axis space, proportional to its weight.
    Fill(u32),
    /// Exactly this many design units of width, whichever axis that is.
    Width(Dip),
    /// Exactly this many design units of height, whichever axis that is.
    Height(Dip),
}

/// One entry of a [`Group`]: a leaf or a nested group, with its sizing.
#[derive(Clone, Debug)]
pub struct Item<K> {
    content: Content<K>,
    sizing: Sizing,
}

#[derive(Clone, Debug)]
enum Content<K> {
    Leaf(K),
    Group(Group<K>),
}

impl<K: Copy> Item<K> {
    /// A leaf keyed by `key`, at its natural size.
    pub fn leaf(key: K) -> Item<K> {
        Item {
            content: Content::Leaf(key),
            sizing: Sizing::Auto,
        }
    }

    /// A nested group, sharing leftover space equally by default.
    pub fn group(group: Group<K>) -> Item<K> {
        Item {
            content: Content::Group(group),
            sizing: Sizing::Fill(1),
        }
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

    fn is_visible(&self, leaf: &dyn Fn(&K) -> Leaf) -> bool {
        match &self.content {
            Content::Leaf(key) => leaf(key).visible,
            Content::Group(group) => group.items.iter().any(|item| item.is_visible(leaf)),
        }
    }

    fn slot(&self, direction: StackDirection, leaf: &dyn Fn(&K) -> Leaf) -> StackSlot {
        match self.sizing {
            Sizing::Fill(weight) => StackSlot::Fill(weight),
            Sizing::Fixed(size) => StackSlot::Fixed(size),
            Sizing::Min(size) => StackSlot::Min(size),
            Sizing::Width(size) if direction == StackDirection::Horizontal => {
                StackSlot::Fixed(size)
            }
            Sizing::Height(size) if direction == StackDirection::Vertical => StackSlot::Fixed(size),
            // A named-axis size on the cross axis leaves the main axis at its
            // natural size.
            Sizing::Auto | Sizing::Width(_) | Sizing::Height(_) => match &self.content {
                Content::Leaf(key) => {
                    let natural = leaf(key).natural;
                    StackSlot::FixedPx(Px(main(direction, natural).max(0)))
                }
                Content::Group(_) => StackSlot::Fill(1),
            },
        }
    }

    /// The cross-axis extent this item asked for, if any.
    fn cross_extent(&self, direction: StackDirection) -> Option<Dip> {
        match (self.sizing, direction) {
            (Sizing::Width(size), StackDirection::Vertical) => Some(size),
            (Sizing::Height(size), StackDirection::Horizontal) => Some(size),
            _ => None,
        }
    }

    fn place(&self, rect: Rect, dpi: u32, leaf: &dyn Fn(&K) -> Leaf, out: &mut Vec<(K, Rect)>) {
        match &self.content {
            Content::Leaf(key) => out.push((*key, rect)),
            Content::Group(group) => group.place(rect, dpi, leaf, out),
        }
    }

    fn natural(&self, direction: StackDirection, dpi: u32, leaf: &dyn Fn(&K) -> Leaf) -> Size {
        let content = match &self.content {
            Content::Leaf(key) => leaf(key).natural,
            Content::Group(group) => group.preferred_size(dpi, leaf),
        };
        let px = |value: Dip| value.to_px(dpi).value().max(0);
        match self.sizing {
            Sizing::Width(value) => Size::new(px(value), content.height),
            Sizing::Height(value) => Size::new(content.width, px(value)),
            sizing => {
                let main_extent = match sizing {
                    Sizing::Fixed(value) => px(value),
                    Sizing::Min(value) => main(direction, content).max(px(value)),
                    // A fill item has no natural extent of its own.
                    Sizing::Fill(_) => 0,
                    _ => main(direction, content),
                };
                with_main(direction, content, main_extent)
            }
        }
    }
}

/// A row or column of [`Item`]s.
#[derive(Clone, Debug)]
pub struct Group<K> {
    direction: StackDirection,
    spacing: Dip,
    margins: Insets,
    items: Vec<Item<K>>,
}

impl<K: Copy> Group<K> {
    /// A group that places its items left to right.
    pub const fn row() -> Group<K> {
        Group::new(StackDirection::Horizontal)
    }

    /// A group that places its items top to bottom.
    pub const fn column() -> Group<K> {
        Group::new(StackDirection::Vertical)
    }

    const fn new(direction: StackDirection) -> Group<K> {
        Group {
            direction,
            spacing: Dip(0.0),
            margins: Insets::new(Dip(0.0), Dip(0.0), Dip(0.0), Dip(0.0)),
            items: Vec::new(),
        }
    }

    /// The gap between adjacent items, in design units.
    pub const fn spacing(mut self, spacing: Dip) -> Group<K> {
        self.spacing = spacing;
        self
    }

    /// Margins inside the parent, in design units.
    pub const fn margins(mut self, margins: Insets) -> Group<K> {
        self.margins = margins;
        self
    }

    /// Appends an item.
    pub fn push(mut self, item: Item<K>) -> Group<K> {
        self.items.push(item);
        self
    }

    /// Lays the tree out inside `rect`, returning one rectangle per visible
    /// leaf, in tree order.
    pub fn compute(&self, rect: Rect, dpi: u32, leaf: &dyn Fn(&K) -> Leaf) -> Vec<(K, Rect)> {
        let mut out = Vec::new();
        self.place(rect, dpi, leaf, &mut out);
        out
    }

    fn stack(&self) -> Stack {
        match self.direction {
            StackDirection::Horizontal => Stack::horizontal(),
            StackDirection::Vertical => Stack::vertical(),
        }
        .spacing(self.spacing)
        .margins(self.margins)
    }

    fn place(&self, rect: Rect, dpi: u32, leaf: &dyn Fn(&K) -> Leaf, out: &mut Vec<(K, Rect)>) {
        let visible: Vec<&Item<K>> = self
            .items
            .iter()
            .filter(|item| item.is_visible(leaf))
            .collect();
        if visible.is_empty() {
            return;
        }
        let stack = visible.iter().fold(self.stack(), |stack, item| {
            stack.push(item.slot(self.direction, leaf))
        });
        for (item, area) in visible.iter().zip(stack.split(rect, dpi)) {
            let area = match item.cross_extent(self.direction) {
                Some(size) => cross_rect(area, self.direction, size.to_px(dpi).value()),
                None => area,
            };
            item.place(area, dpi, leaf, out);
        }
    }

    /// The size the group's content wants, in device pixels at `dpi`: along the
    /// main axis the margins plus each visible item's natural extent plus one
    /// gap between neighbours; across it the margins plus the largest natural
    /// cross extent. A [`Sizing::Fill`] item contributes no natural extent.
    pub fn preferred_size(&self, dpi: u32, leaf: &dyn Fn(&K) -> Leaf) -> Size {
        let naturals: Vec<Size> = self
            .items
            .iter()
            .filter(|item| item.is_visible(leaf))
            .map(|item| item.natural(self.direction, dpi, leaf))
            .collect();
        self.stack().preferred_size(&naturals, dpi)
    }
}

/// The extent of `size` along `direction`.
fn main(direction: StackDirection, size: Size) -> i32 {
    match direction {
        StackDirection::Horizontal => size.width,
        StackDirection::Vertical => size.height,
    }
}

/// `size` with its extent along `direction` replaced by `extent`.
fn with_main(direction: StackDirection, size: Size, extent: i32) -> Size {
    match direction {
        StackDirection::Horizontal => Size::new(extent, size.height),
        StackDirection::Vertical => Size::new(size.width, extent),
    }
}

/// Narrows `rect` to `extent` pixels along the cross axis, keeping the start
/// edge (the top for a row, the left for a column).
fn cross_rect(rect: Rect, direction: StackDirection, extent: i32) -> Rect {
    match direction {
        StackDirection::Horizontal => Rect::new(
            rect.left,
            rect.top,
            rect.right,
            (rect.top + extent).min(rect.bottom),
        ),
        StackDirection::Vertical => Rect::new(
            rect.left,
            rect.top,
            (rect.left + extent).min(rect.right),
            rect.bottom,
        ),
    }
}

#[cfg(test)]
mod tests;
