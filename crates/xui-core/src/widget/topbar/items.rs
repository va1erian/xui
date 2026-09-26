#![forbid(unsafe_code)]

//! The [`TopBar`](super::TopBar) item model and its horizontal layout.
//!
//! Items keep their natural width; a [`Spacer`](Kind::Spacer) soaks up the
//! leftover width in proportion to its weight, so a bar can push trailing
//! items to the right without absolute positions.

use std::cell::RefCell;
use std::rc::Rc;

use crate::geometry::Rect;
use crate::units::Dip;

use super::{Glyph, TopBarId};

/// The design size of an icon button or a toggle.
pub(super) const ITEM: Dip = Dip(36.0);
/// The design width of a slider item.
pub(super) const SLIDER: Dip = Dip(120.0);
/// The design width budget of one label character.
///
/// The portable [`Canvas`](crate::Canvas) does not measure text, so a label's
/// width is estimated from its character count; a label a pixel or two wide is
/// still hit-tested by its real cell.
pub(super) const LABEL_CHAR: Dip = Dip(7.0);
/// The horizontal inset of a label's text from its cell edge.
pub(super) const PADDING: Dip = Dip(6.0);
/// The thumb radius of a slider item.
pub(super) const THUMB: Dip = Dip(6.0);
/// The track thickness of a slider item.
pub(super) const TRACK: Dip = Dip(3.0);

/// What an [`Item`] is.
#[derive(Clone, Debug)]
pub(super) enum Kind {
    /// A clickable icon button.
    Icon(Glyph),
    /// A latching icon button.
    Toggle {
        /// The button's icon.
        glyph: Glyph,
        /// Whether it is checked.
        checked: bool,
    },
    /// A non-interactive text label.
    Label(String),
    /// A draggable range control.
    Slider {
        /// The smallest value.
        min: f64,
        /// The largest value.
        max: f64,
        /// The current value.
        value: f64,
    },
    /// A flexible gap that shares leftover width by weight.
    Spacer(u32),
}

/// One entry of a [`TopBar`](super::TopBar).
#[derive(Clone, Debug)]
pub(super) struct Item {
    /// The item's opaque handle.
    pub(super) id: TopBarId,
    /// Whether the item accepts input and paints at full strength.
    pub(super) enabled: bool,
    /// The item's hover tooltip, if any.
    pub(super) tooltip: Option<String>,
    /// What the item is.
    pub(super) kind: Kind,
}

/// Whether `kind` accepts pointer input.
pub(super) fn is_interactive(kind: &Kind) -> bool {
    !matches!(kind, Kind::Label(_) | Kind::Spacer(_))
}

/// An item's natural width in device pixels.
pub(super) fn item_width(kind: &Kind, dpi: u32) -> i32 {
    match kind {
        Kind::Icon(_) | Kind::Toggle { .. } => ITEM.to_px(dpi).value(),
        Kind::Label(text) => {
            let char_width = LABEL_CHAR.to_px(dpi).value().max(1);
            char_width * text.chars().count() as i32 + 2 * PADDING.to_px(dpi).value()
        }
        Kind::Slider { .. } => SLIDER.to_px(dpi).value(),
        Kind::Spacer(_) => 0,
    }
}

/// Visits every item with the cell it occupies, left to right.
///
/// Leftover width is split between spacers by cumulative rounding, so the
/// spacers sum to exactly the leftover and the cells tile the bar without gaps.
pub(super) fn each_rect(
    items: &[Item],
    bounds: Rect,
    dpi: u32,
    mut visit: impl FnMut(usize, Rect),
) {
    let fixed: i32 = items.iter().map(|item| item_width(&item.kind, dpi)).sum();
    let total_weight: u32 = items
        .iter()
        .map(|item| match item.kind {
            Kind::Spacer(weight) => weight,
            _ => 0,
        })
        .sum();
    let extra = (bounds.width() - fixed).max(0);
    let mut cursor = bounds.left;
    let mut accumulated = 0i64;
    let mut assigned = 0i64;
    for (index, item) in items.iter().enumerate() {
        let width = match &item.kind {
            Kind::Spacer(weight) if *weight > 0 && extra > 0 => {
                accumulated += i64::from(*weight) * i64::from(extra);
                let boundary = accumulated / i64::from(total_weight);
                let width = (boundary - assigned) as i32;
                assigned = boundary;
                width
            }
            _ => item_width(&item.kind, dpi),
        };
        let rect = Rect::new(cursor, bounds.top, cursor + width, bounds.bottom);
        visit(index, rect);
        cursor += width;
    }
}

/// The index of the item whose cell contains `x`, if any.
pub(super) fn item_at(items: &[Item], bounds: Rect, dpi: u32, x: i32) -> Option<usize> {
    let mut found = None;
    each_rect(items, bounds, dpi, |index, rect| {
        if found.is_none() && x >= rect.left && x < rect.right {
            found = Some(index);
        }
    });
    found
}

/// The cell of item `index`, if it exists.
pub(super) fn item_rect(items: &[Item], bounds: Rect, dpi: u32, index: usize) -> Option<Rect> {
    let mut found = None;
    each_rect(items, bounds, dpi, |current, rect| {
        if current == index {
            found = Some(rect);
        }
    });
    found
}

/// The index of an enabled, interactive item under `x`, if any.
pub(super) fn hit_interactive(items: &[Item], bounds: Rect, dpi: u32, x: i32) -> Option<usize> {
    item_at(items, bounds, dpi, x)
        .filter(|index| is_interactive(&items[*index].kind) && items[*index].enabled)
}

/// Moves slider `index` to the value at `x`, returning the value and whether it
/// changed. A non-slider item returns `None`.
pub(super) fn set_slider(
    items: &Rc<RefCell<Vec<Item>>>,
    index: usize,
    bounds: Rect,
    dpi: u32,
    x: i32,
) -> Option<(f64, bool)> {
    let rect = item_rect(&items.borrow()[..], bounds, dpi, index)?;
    let thumb = THUMB.to_px(dpi).value().max(1);
    let left = rect.left + thumb;
    let right = rect.right - thumb;
    let span = (right - left).max(1) as f64;
    let fraction = ((x - left) as f64 / span).clamp(0.0, 1.0);
    let mut borrowed = items.borrow_mut();
    let item = borrowed.get_mut(index)?;
    let Kind::Slider { min, max, value } = &mut item.kind else {
        return None;
    };
    let new = *min + fraction * (*max - *min);
    let changed = (new - *value).abs() > f64::EPSILON;
    *value = new;
    Some((new, changed))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(kind: Kind) -> Item {
        Item {
            id: TopBarId::new(0),
            enabled: true,
            tooltip: None,
            kind,
        }
    }

    fn rects(items: &[Item], bounds: Rect, dpi: u32) -> Vec<Rect> {
        let mut out = Vec::new();
        each_rect(items, bounds, dpi, |_, rect| out.push(rect));
        out
    }

    #[test]
    fn fixed_items_keep_their_width_and_ignore_surplus() {
        let items = [
            item(Kind::Icon(Glyph::Menu)),
            item(Kind::Label("ab".into())),
        ];
        let bounds = Rect::new(0, 0, 200, 24);
        let cells = rects(&items, bounds, 96);
        assert_eq!(cells[0], Rect::new(0, 0, 36, 24));
        assert_eq!(cells[1].left, 36, "the label starts after the icon");
        assert_eq!(cells[1].width(), 7 * 2 + 12);
    }

    #[test]
    fn spacers_share_the_leftover_width() {
        let items = [
            item(Kind::Icon(Glyph::Menu)),
            item(Kind::Spacer(1)),
            item(Kind::Icon(Glyph::Close)),
            item(Kind::Spacer(1)),
        ];
        let bounds = Rect::new(0, 0, 200, 24);
        let cells = rects(&items, bounds, 96);
        assert_eq!(cells[0], Rect::new(0, 0, 36, 24));
        assert_eq!(cells[1], Rect::new(36, 0, 100, 24));
        assert_eq!(cells[2], Rect::new(100, 0, 136, 24));
        assert_eq!(cells[3], Rect::new(136, 0, 200, 24));
    }

    #[test]
    fn weighted_spacers_split_proportionally() {
        let items = [
            item(Kind::Spacer(1)),
            item(Kind::Icon(Glyph::Menu)),
            item(Kind::Spacer(3)),
        ];
        let bounds = Rect::new(0, 0, 136, 24);
        let cells = rects(&items, bounds, 96);
        assert_eq!(cells[0].width(), 25);
        assert_eq!(cells[2].width(), 75);
        assert_eq!(cells[0].left + cells[0].width(), 25);
        assert_eq!(cells[2].right, 136);
    }

    #[test]
    fn hit_testing_skips_labels_and_spacers() {
        let items = [
            item(Kind::Spacer(1)),
            item(Kind::Label("hi".into())),
            item(Kind::Icon(Glyph::Search)),
        ];
        let bounds = Rect::new(0, 0, 120, 24);
        let label = item_rect(&items, bounds, 96, 1).unwrap();
        assert_eq!(item_at(&items, bounds, 96, label.left + 1), Some(1));
        assert_eq!(hit_interactive(&items, bounds, 96, label.left + 1), None);
        let icon = item_rect(&items, bounds, 96, 2).unwrap();
        assert_eq!(hit_interactive(&items, bounds, 96, icon.left + 1), Some(2));
    }
}
