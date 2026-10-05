#![forbid(unsafe_code)]

//! [`Item`]: one entry of a [`Group`] with its sizing and placement.

use super::{Constraints, Group, LeafFn, Out, Sizing, main, with_main};
use crate::geometry::{Rect, Size};
use crate::layout::{Anchor, StackDirection};
use crate::units::Dip;

/// One entry of a [`Group`]: a leaf, a nested group, or a leaf that frames a
/// nested group, with its sizing and placement.
#[derive(Clone, Debug)]
pub struct Item<K> {
    pub(super) content: Content<K>,
    pub(super) sizing: Sizing,
    pub(super) align: Option<super::Align>,
    pub(super) max_width: Option<Dip>,
    pub(super) max_height: Option<Dip>,
    pub(super) span: usize,
    pub(super) at: Option<[Dip; 4]>,
    pub(super) anchor: Anchor,
}

#[derive(Clone, Debug)]
pub(super) enum Content<K> {
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
            at: None,
            anchor: Anchor::TopLeft,
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
    /// leaf reports as [`Leaf::content`](super::Leaf::content).
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

    /// Places the item across its parent's main axis (in a grid or a layered
    /// group, within its area on both axes) instead of the parent's default.
    pub fn align(mut self, align: super::Align) -> Item<K> {
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

    /// In a [`Group::absolute`], places the item at `x`, `y`, `width` wide and
    /// `height` tall, in design units from the group's inner top-left corner.
    /// Other groups ignore it.
    pub fn at(mut self, x: Dip, y: Dip, width: Dip, height: Dip) -> Item<K> {
        self.at = Some([x, y, width, height]);
        self
    }

    /// In a [`Group::absolute`], how the item follows the group when it is
    /// laid out at a size other than its design size. The default,
    /// [`Anchor::TopLeft`], keeps it where [`at`](Self::at) put it.
    pub fn anchor(mut self, anchor: Anchor) -> Item<K> {
        self.anchor = anchor;
        self
    }

    pub(super) fn is_visible(&self, leaf: LeafFn<'_, K>, dpi: u32) -> bool {
        match &self.content {
            Content::Leaf(key) | Content::Framed(key, _) => {
                leaf(key, Constraints::unbounded(dpi)).visible
            }
            Content::Group(group) => group.items.iter().any(|item| item.is_visible(leaf, dpi)),
        }
    }

    /// The content's natural size within `constraints`, before the item's
    /// own sizing applies.
    pub(super) fn measure(&self, constraints: Constraints, leaf: LeafFn<'_, K>) -> Size {
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
    pub(super) fn natural(
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

    /// The item's own size within `constraints` in a group with no main axis
    /// to share (a wrap measures along `direction`, a layered group along the
    /// vertical): like [`natural`](Self::natural), but a fill item keeps its
    /// content's size.
    pub(super) fn own_size(
        &self,
        direction: StackDirection,
        constraints: Constraints,
        leaf: LeafFn<'_, K>,
    ) -> Size {
        match self.sizing {
            Sizing::Fill(_) => self.capped(self.measure(constraints, leaf), constraints.dpi),
            _ => self.natural(direction, constraints, leaf),
        }
    }

    /// `size` with the item's caps applied.
    pub(super) fn capped(&self, size: Size, dpi: u32) -> Size {
        let cap = |value: i32, max: Option<Dip>| match max {
            Some(max) => value.min(max.to_px(dpi).value().max(0)),
            None => value,
        };
        Size::new(
            cap(size.width, self.max_width),
            cap(size.height, self.max_height),
        )
    }

    pub(super) fn place(&self, rect: Rect, dpi: u32, leaf: LeafFn<'_, K>, out: &mut Out<K>) {
        match &self.content {
            Content::Leaf(key) => out.leaf(*key, rect),
            Content::Group(group) => group.place(rect, dpi, leaf, out),
            Content::Framed(key, group) => {
                out.leaf(*key, rect);
                let insets = leaf(key, Constraints::unbounded(dpi)).content;
                out.nested(|out| group.place(insets.apply(rect, dpi), dpi, leaf, out));
            }
        }
    }
}
