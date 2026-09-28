#![forbid(unsafe_code)]

//! [`Toolbar`](super::Toolbar)'s painter.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use crate::backend::{Canvas, TextStyle};
use crate::geometry::{Point, Rect};
use crate::icon::draw_icon;
use crate::theme::Theme;
use crate::units::Dip;

use super::{Item, cell_span};

/// The corner radius of an item's highlight.
const RADIUS: f32 = 4.0;
/// The design size of an item's label.
const TEXT_SIZE: Dip = Dip(12.0);
/// The gap between an icon and its label, in device pixels.
const GAP: i32 = 6;
/// The largest icon side, in device pixels.
const ICON_MAX: i32 = 20;

/// Paints the strip's items into `canvas`.
pub(super) fn paint(
    canvas: &mut dyn Canvas,
    items: &Rc<RefCell<Vec<Item>>>,
    hover: &Cell<Option<usize>>,
    pressed: &Cell<Option<usize>>,
    enabled: &Cell<bool>,
    selected: &Cell<bool>,
    theme: Theme,
) {
    let bounds = canvas.bounds();
    let dpi = canvas.dpi();
    canvas.clear(theme.background);

    let items = items.borrow();
    let count = items.len();
    let enabled = enabled.get();
    for (index, item) in items.iter().enumerate() {
        let (start, end) = cell_span(bounds.width(), count, index);
        if end <= start {
            // A strip narrower than its item count leaves some cells empty.
            continue;
        }
        let left = bounds.left + start;
        let rect = Rect::new(left, bounds.top, bounds.left + end, bounds.bottom);
        if pressed.get() == Some(index) {
            canvas.fill_rounded_rect(rect, RADIUS, theme.pressed);
        } else if hover.get() == Some(index) {
            canvas.fill_rounded_rect(rect, RADIUS, theme.hover);
        }
        if index > 0 {
            canvas.draw_line(
                Point::new(left, bounds.top),
                Point::new(left, bounds.bottom),
                theme.border,
                1.0,
            );
        }
        let color = if enabled {
            theme.text
        } else {
            theme.text_disabled
        };
        let (icon_rect, text_rect) = content_rects(rect, item.icon.is_some(), item.label.is_none());
        if let Some(icon) = item.icon
            && let Some(icon_rect) = icon_rect
        {
            draw_icon(canvas, icon, icon_rect, color, dpi);
        }
        if let Some(label) = &item.label {
            let style = TextStyle::new(color, TEXT_SIZE).centered().middle();
            canvas.draw_text(label, text_rect, &style);
        }
    }

    if selected.get() {
        canvas.stroke_rect(bounds, theme.accent, 2.0);
    }
}

/// The icon and label rectangles inside an item's cell.
///
/// With no icon the label keeps the whole cell; with an icon and a label the
/// icon sits at the leading edge and the label is centred in what remains; with
/// an icon and no label the icon is centred.
fn content_rects(bounds: Rect, has_icon: bool, text_empty: bool) -> (Option<Rect>, Rect) {
    if !has_icon {
        return (None, bounds);
    }
    let side = (bounds.height() * 2 / 3).clamp(8, ICON_MAX);
    if text_empty {
        let left = bounds.left + (bounds.width() - side) / 2;
        let top = bounds.top + (bounds.height() - side) / 2;
        return (Some(Rect::new(left, top, left + side, top + side)), bounds);
    }
    let top = bounds.top + (bounds.height() - side) / 2;
    let icon_rect = Rect::new(bounds.left + GAP, top, bounds.left + GAP + side, top + side);
    let text_rect = Rect::new(
        icon_rect.right + GAP,
        bounds.top,
        bounds.right,
        bounds.bottom,
    );
    (Some(icon_rect), text_rect)
}
