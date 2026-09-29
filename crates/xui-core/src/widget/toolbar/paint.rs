#![forbid(unsafe_code)]

//! [`Toolbar`](super::Toolbar)'s painter.

use std::cell::Cell;

use crate::backend::Canvas;
use crate::geometry::Rect;
use crate::icon::draw_icon;
use crate::theme::Theme;
use crate::units::Dip;

use super::layout::{self, ICON_GAP, PADDING, SEPARATOR_WIDTH};
use super::strip::State;

/// The corner radius of an item's highlight.
const RADIUS: f32 = 4.0;
/// How far a button's highlight is pulled in from its span on every side, so
/// neighbouring highlights do not touch.
const HIGHLIGHT_INSET: Dip = Dip(2.0);
/// The empty room above and below a separator line.
const SEPARATOR_INSET: Dip = Dip(10.0);

/// Paints the strip's items and separators into `canvas`.
pub(super) fn paint(canvas: &mut dyn Canvas, state: &State, selected: &Cell<bool>, theme: Theme) {
    let bounds = canvas.bounds();
    let dpi = canvas.dpi();
    canvas.clear(theme.background);

    let color = if state.enabled.get() {
        theme.text
    } else {
        theme.text_disabled
    };
    let style = layout::label_style(color);
    let entries = state.entries.borrow();
    let strip = layout::compute(
        &entries,
        state.mode.get(),
        (bounds.width(), bounds.height()),
        dpi,
        &mut |text| canvas.measure_text(text, &style).width,
    );

    let inset = SEPARATOR_INSET.to_px(dpi).0.min(bounds.height() / 2);
    let line = SEPARATOR_WIDTH.to_px(dpi).0.max(1);
    for &x in &strip.separators {
        let left = bounds.left + x;
        let rect = Rect::new(left, bounds.top + inset, left + line, bounds.bottom - inset);
        canvas.fill_rect(rect, theme.border);
    }

    let highlight = HIGHLIGHT_INSET.to_px(dpi).0;
    for (index, item) in entries.items().enumerate() {
        let (start, end) = strip.items[index];
        if end <= start {
            // Clipped, or no room: neither painted nor hit.
            continue;
        }
        let rect = Rect::new(
            bounds.left + start,
            bounds.top,
            bounds.left + end,
            bounds.bottom,
        );
        let button = rect.shrink(highlight.min(rect.width() / 2).min(rect.height() / 2));
        if state.pressed.get() == Some(index) {
            canvas.fill_rounded_rect(button, RADIUS, theme.pressed);
        } else if state.hover.get() == Some(index) {
            canvas.fill_rounded_rect(button, RADIUS, theme.hover);
        }
        let (icon_rect, text_rect) = content_rects(rect, item.icon.is_some(), &item.label, dpi);
        if let Some(icon) = item.icon {
            draw_icon(canvas, icon, icon_rect, color, dpi);
        }
        if let Some(label) = &item.label {
            canvas.draw_text(label, text_rect, &style);
        }
    }

    if selected.get() {
        canvas.stroke_rect(bounds, theme.accent, 2.0);
    }
}

/// The icon and label rectangles inside an item's `span`.
///
/// With no icon the label keeps the whole span; with an icon and a label the
/// icon sits one padding in from the leading edge and the label is centred
/// between the icon and the trailing padding (a compact button is exactly that
/// wide, so it lands flush); with an icon and no label the icon is centred.
fn content_rects(span: Rect, has_icon: bool, label: &Option<String>, dpi: u32) -> (Rect, Rect) {
    if !has_icon {
        return (Rect::default(), span);
    }
    let side = layout::icon_side(span.height(), dpi);
    let top = span.top + (span.height() - side) / 2;
    if label.is_none() {
        let left = span.left + (span.width() - side) / 2;
        return (Rect::new(left, top, left + side, top + side), span);
    }
    let pad = PADDING.to_px(dpi).0;
    let left = span.left + pad;
    let icon = Rect::new(left, top, left + side, top + side);
    let text = Rect::new(
        icon.right + ICON_GAP.to_px(dpi).0,
        span.top,
        span.right - pad,
        span.bottom,
    );
    (icon, text)
}
