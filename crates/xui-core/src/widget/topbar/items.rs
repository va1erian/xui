#![forbid(unsafe_code)]

//! The [`TopBar`](super::TopBar) item model and its horizontal layout.
//!
//! Items keep their natural width; a [`Spacer`](Kind::Spacer) or an item
//! given an explicit [`Width::Expand`] soaks up the leftover width in
//! proportion to its weight, and [`Width::Fixed`] pins an item's width, so a
//! bar can stretch a seek slider or push trailing items to the right without
//! absolute positions.

use std::cell::RefCell;
use std::rc::Rc;

use crate::geometry::Rect;
use crate::icon::IconRef;
use crate::units::Dip;

use super::TopBarId;

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
    Icon(IconRef),
    /// A latching icon button.
    Toggle {
        /// The button's icon.
        glyph: IconRef,
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

/// The width an item was asked to take, overriding its natural width.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Width {
    /// A fixed design width.
    Fixed(Dip),
    /// A share of the bar's leftover width, by weight.
    Expand(u32),
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
    /// An explicit width, or `None` to use the kind's natural width.
    pub(super) width: Option<Width>,
    /// What the item is.
    pub(super) kind: Kind,
}

/// Whether `kind` accepts pointer input.
pub(super) fn is_interactive(kind: &Kind) -> bool {
    !matches!(kind, Kind::Label(_) | Kind::Spacer(_))
}

/// An item's natural width in device pixels.
fn natural_width(kind: &Kind, dpi: u32) -> i32 {
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

/// The bar's natural width: every item at the width it claims before the
/// leftover is shared.
pub(super) fn bar_width(items: &[Item], dpi: u32) -> i32 {
    items.iter().map(|item| fixed_width(item, dpi)).sum()
}

/// The width `item` claims before the leftover is shared: zero when it expands,
/// so its natural width does not eat into the space meant to be distributed.
fn fixed_width(item: &Item, dpi: u32) -> i32 {
    match item.width {
        Some(Width::Expand(_)) => 0,
        Some(Width::Fixed(width)) => width.to_px(dpi).value().max(0),
        None => natural_width(&item.kind, dpi),
    }
}

/// The weight with which `item` shares the bar's leftover width, if it does.
/// A spacer and an explicitly expanding item both take part.
fn share_weight(item: &Item) -> Option<u32> {
    match item.width {
        Some(Width::Expand(weight)) => Some(weight.max(1)),
        Some(Width::Fixed(_)) => None,
        None => match item.kind {
            Kind::Spacer(weight) => Some(weight.max(1)),
            _ => None,
        },
    }
}

/// Visits every item with the cell it occupies, left to right.
///
/// Leftover width is split between spacers and expanding items by cumulative
/// rounding, so they sum to exactly the leftover and the cells tile the bar
/// without gaps.
pub(super) fn each_rect(
    items: &[Item],
    bounds: Rect,
    dpi: u32,
    mut visit: impl FnMut(usize, Rect),
) {
    let fixed: i32 = items.iter().map(|item| fixed_width(item, dpi)).sum();
    let total_weight: u32 = items.iter().filter_map(share_weight).sum();
    let extra = (bounds.width() - fixed).max(0);
    let mut cursor = bounds.left;
    let mut accumulated = 0i64;
    let mut assigned = 0i64;
    for (index, item) in items.iter().enumerate() {
        let width = match share_weight(item) {
            Some(weight) if extra > 0 => {
                accumulated += i64::from(weight) * i64::from(extra);
                let boundary = accumulated / i64::from(total_weight);
                let width = (boundary - assigned) as i32;
                assigned = boundary;
                width
            }
            _ => fixed_width(item, dpi),
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
mod tests;
