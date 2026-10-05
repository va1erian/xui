#![forbid(unsafe_code)]

//! [`Placement`]: an absolute item's design rectangle and anchor, shared so
//! they can change after the layout is built.

use std::cell::Cell;
use std::rc::Rc;

use super::Anchor;
use crate::units::Dip;

/// Where an item of an absolute layout sits: its design rectangle (`x`, `y`,
/// width and height, in design units from the layout's inner top-left) and
/// its [`Anchor`], in a shared cell.
///
/// The layout reads it each time it places the item, so a designer dragging a
/// control or a script setting its `left` moves it for good: change the
/// placement, then lay the window out again ([`Ui::relayout`]). Clones share
/// one cell.
///
/// [`Ui::relayout`]: crate::app::Ui::relayout
#[derive(Clone, Debug)]
pub struct Placement(Rc<Cell<([Dip; 4], Anchor)>>);

impl Placement {
    /// A placement at `x`, `y`, `width` wide and `height` tall, following its
    /// layout with `anchor`.
    pub fn new(x: Dip, y: Dip, width: Dip, height: Dip, anchor: Anchor) -> Placement {
        Placement(Rc::new(Cell::new(([x, y, width, height], anchor))))
    }

    /// The design rectangle, as `[x, y, width, height]`.
    pub fn rect(&self) -> [Dip; 4] {
        self.0.get().0
    }

    /// The anchor.
    pub fn anchor(&self) -> Anchor {
        self.0.get().1
    }

    /// Moves and resizes the item to `rect`, `[x, y, width, height]`.
    pub fn set_rect(&self, rect: [Dip; 4]) {
        let (_, anchor) = self.0.get();
        self.0.set((rect, anchor));
    }

    /// Makes the item follow its layout with `anchor`.
    pub fn set_anchor(&self, anchor: Anchor) {
        let (rect, _) = self.0.get();
        self.0.set((rect, anchor));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clones_share_one_cell() {
        let placement = Placement::new(Dip(1.0), Dip(2.0), Dip(3.0), Dip(4.0), Anchor::TopLeft);
        let shared = placement.clone();
        shared.set_rect([Dip(5.0), Dip(6.0), Dip(7.0), Dip(8.0)]);
        shared.set_anchor(Anchor::Fill);
        assert_eq!(placement.rect(), [Dip(5.0), Dip(6.0), Dip(7.0), Dip(8.0)]);
        assert_eq!(placement.anchor(), Anchor::Fill);
    }
}
