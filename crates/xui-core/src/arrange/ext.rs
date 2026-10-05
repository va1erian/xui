#![forbid(unsafe_code)]

//! [`LayoutExt`]: sizing and placement for anything a layout holds.

use super::{Align, Anchor, Entry, IntoEntry};
use crate::layout::Sizing;
use crate::units::Dip;

/// Sizing and placement for anything a [`Layout`](super::Layout) can hold:
/// `edit().fill(1)`, `row().fixed(40)`, `label("x").align(Align::End)`.
pub trait LayoutExt<M: 'static>: IntoEntry<M> + Sized {
    /// Sizes the entry with `sizing`.
    fn sized(self, sizing: Sizing) -> Entry<M> {
        let mut entry = self.into_entry();
        entry.sizing = sizing;
        entry
    }

    /// A share of the parent's leftover space, by `weight`. In a grid, makes
    /// the entry's row fill.
    fn fill(self, weight: u32) -> Entry<M> {
        self.sized(Sizing::Fill(weight))
    }

    /// Exactly `size` along the parent's main axis.
    fn fixed(self, size: impl Into<Dip>) -> Entry<M> {
        self.sized(Sizing::Fixed(size.into()))
    }

    /// At least `size` along the parent's main axis.
    fn min(self, size: impl Into<Dip>) -> Entry<M> {
        self.sized(Sizing::Min(size.into()))
    }

    /// Exactly `size` wide: the main axis in a row, the cross axis in a column.
    fn width(self, size: impl Into<Dip>) -> Entry<M> {
        self.sized(Sizing::Width(size.into()))
    }

    /// Exactly `size` tall: the main axis in a column, the cross axis in a row.
    fn height(self, size: impl Into<Dip>) -> Entry<M> {
        self.sized(Sizing::Height(size.into()))
    }

    /// Where the entry sits across its parent's main axis (in a grid or an
    /// overlay, within its area on both axes), instead of the parent's
    /// [`Layout::align`](super::Layout::align).
    fn align(self, align: Align) -> Entry<M> {
        let mut entry = self.into_entry();
        entry.align = Some(align);
        entry
    }

    /// Caps the entry's width at `width` design units.
    fn max_width(self, width: impl Into<Dip>) -> Entry<M> {
        let mut entry = self.into_entry();
        entry.max_width = Some(width.into());
        entry
    }

    /// Caps the entry's height at `height` design units.
    fn max_height(self, height: impl Into<Dip>) -> Entry<M> {
        let mut entry = self.into_entry();
        entry.max_height = Some(height.into());
        entry
    }

    /// In a grid, covers `columns` columns.
    fn span(self, columns: usize) -> Entry<M> {
        let mut entry = self.into_entry();
        entry.span = columns.max(1);
        entry
    }

    /// In an [`absolute`](super::absolute) layout, places the entry at `x`,
    /// `y`, `width` wide and `height` tall, in design units from the layout's
    /// inner top-left corner. Other layouts ignore it.
    fn at(
        self,
        x: impl Into<Dip>,
        y: impl Into<Dip>,
        width: impl Into<Dip>,
        height: impl Into<Dip>,
    ) -> Entry<M> {
        let mut entry = self.into_entry();
        entry.at = Some([x.into(), y.into(), width.into(), height.into()]);
        entry
    }

    /// In an [`absolute`](super::absolute) layout, how the entry follows the
    /// layout when it is larger or smaller than its design size: pinned to a
    /// corner or an edge, or stretched. The default is [`Anchor::TopLeft`].
    fn anchor(self, anchor: Anchor) -> Entry<M> {
        let mut entry = self.into_entry();
        entry.anchor = Some(anchor);
        entry
    }
}

impl<M: 'static, T: IntoEntry<M>> LayoutExt<M> for T {}
