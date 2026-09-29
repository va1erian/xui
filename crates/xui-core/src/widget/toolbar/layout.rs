#![forbid(unsafe_code)]

//! The toolbar's one layout function.
//!
//! [`compute`] turns the entries, the strip's size and the DPI into each item's
//! span and each separator's position. Painting, hit-testing, hover and
//! tooltips all read its result, so what is drawn is what is hit.
//!
//! The layout is recomputed on every paint and every mouse event rather than
//! cached: items change rarely and a strip holds a handful of them, and a cache
//! would need invalidating on every item, bounds, DPI and text-style change.
//! The only theme input to a measurement is [`label_style`]'s size and family,
//! which are constants, so a theme change cannot alter a width.

use crate::backend::TextStyle;
use crate::color::Color;
use crate::units::Dip;

use super::strip::{Entries, Entry, Item};

/// The design size of an item's label.
pub(super) const TEXT_SIZE: Dip = Dip(12.0);
/// The design side of an item's icon.
pub(super) const ICON_SIZE: Dip = Dip(16.0);
/// The padding at each end of a button that has a label.
pub(super) const PADDING: Dip = Dip(8.0);
/// The gap between an icon and its label.
pub(super) const ICON_GAP: Dip = Dip(6.0);
/// The thickness of a separator line.
pub(super) const SEPARATOR_WIDTH: Dip = Dip(1.0);
/// The empty room on each side of a separator line.
pub(super) const SEPARATOR_MARGIN: Dip = Dip(4.0);

/// How the strip sizes and places its items.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Mode {
    /// Items are packed from the left, each as wide as its content.
    Compact,
    /// The strip's width is split equally between the items.
    Fill,
}

/// Where the entries land, in strip-local device pixels.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct Layout {
    /// The `[start, end)` span of each item, by item index. A clipped item, or
    /// any item in a strip with no room, has the empty span `(0, 0)`.
    pub(super) items: Vec<(i32, i32)>,
    /// The x of each visible separator line.
    pub(super) separators: Vec<i32>,
}

impl Layout {
    /// The item at strip-local `x`; `None` on a gap, a separator or past the
    /// last item.
    pub(super) fn item_at(&self, x: i32) -> Option<usize> {
        self.items
            .iter()
            .position(|&(start, end)| (start..end).contains(&x))
    }
}

/// The text style of an item's label, used both to draw it and to measure it.
pub(super) fn label_style(color: Color) -> TextStyle {
    TextStyle::new(color, TEXT_SIZE).centered().middle()
}

/// The icon side in device pixels, never larger than the strip is tall.
pub(super) fn icon_side(height: i32, dpi: u32) -> i32 {
    ICON_SIZE.to_px(dpi).0.min(height).max(1)
}

/// The node-local `[start, end)` span of item `index` in a strip `width`
/// pixels wide holding `count` items, split equally ([`Mode::Fill`]).
///
/// Edges are spread proportionally, so every cell stays inside the strip even
/// when it is narrower than the item count (some cells are then empty) and the
/// division remainder is shared out rather than piled onto one cell.
pub(super) fn cell_span(width: i32, count: usize, index: usize) -> (i32, i32) {
    if count == 0 || width <= 0 {
        return (0, 0);
    }
    (edge(width, count, index), edge(width, count, index + 1))
}

/// The proportional edge `i` of `count` cells across `width`.
fn edge(width: i32, count: usize, i: usize) -> i32 {
    (i64::from(width) * i as i64 / count as i64) as i32
}

/// Lays `entries` out in a `width` x `height` strip at `dpi`.
///
/// `measure` returns a label's advance width in device pixels for
/// [`label_style`]; it must not deliver events.
pub(super) fn compute(
    entries: &Entries,
    mode: Mode,
    (width, height): (i32, i32),
    dpi: u32,
    measure: &mut dyn FnMut(&str) -> i32,
) -> Layout {
    let count = entries.item_count();
    let mut layout = Layout {
        items: vec![(0, 0); count],
        separators: Vec::new(),
    };
    if width <= 0 || height <= 0 {
        return layout;
    }
    match mode {
        Mode::Fill => fill(entries, width, &mut layout),
        Mode::Compact => compact(entries, (width, height), dpi, measure, &mut layout),
    }
    layout
}

/// Equal cells; a separator is a line on the boundary before the next item.
fn fill(entries: &Entries, width: i32, layout: &mut Layout) {
    let count = layout.items.len();
    let mut index = 0;
    for entry in entries.all() {
        match entry {
            Entry::Item(_) => {
                layout.items[index] = cell_span(width, count, index);
                index += 1;
            }
            Entry::Separator => {
                let x = edge(width, count.max(1), index);
                if x < width {
                    layout.separators.push(x);
                }
            }
        }
    }
}

/// Left-packed content-sized buttons; stops at the first entry that does not
/// fit completely, so a trailing item is clipped, never squeezed.
fn compact(
    entries: &Entries,
    (width, height): (i32, i32),
    dpi: u32,
    measure: &mut dyn FnMut(&str) -> i32,
    layout: &mut Layout,
) {
    let margin = SEPARATOR_MARGIN.to_px(dpi).0;
    let line = SEPARATOR_WIDTH.to_px(dpi).0.max(1);
    let mut x = 0;
    let mut index = 0;
    for entry in entries.all() {
        match entry {
            Entry::Item(item) => {
                let end = x + button_width(item, height, dpi, measure);
                if end > width {
                    return;
                }
                layout.items[index] = (x, end);
                index += 1;
                x = end;
            }
            Entry::Separator => {
                let end = x + margin + line + margin;
                if end > width {
                    return;
                }
                layout.separators.push(x + margin);
                x = end;
            }
        }
    }
}

/// A compact button's width: square when icon-only, otherwise its content plus
/// padding at both ends.
fn button_width(item: &Item, height: i32, dpi: u32, measure: &mut dyn FnMut(&str) -> i32) -> i32 {
    let Some(label) = &item.label else {
        return height;
    };
    let pad = PADDING.to_px(dpi).0;
    let icon = if item.icon.is_some() {
        icon_side(height, dpi) + ICON_GAP.to_px(dpi).0
    } else {
        0
    };
    pad + icon + measure(label).max(0) + pad
}
