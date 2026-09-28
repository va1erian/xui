#![forbid(unsafe_code)]

//! [`TopBar`](super::TopBar)'s painter.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use crate::backend::{Canvas, TextStyle};
use crate::geometry::{Point, Rect};
use crate::icon::draw_icon;
use crate::theme::Theme;
use crate::units::Dip;

use super::items::{self, Item, Kind, PADDING, THUMB, TRACK};

/// The corner radius of an item's highlight, checked fill and focus ring.
const RADIUS: f32 = 4.0;
/// The design size of a label's text.
const LABEL_SIZE: Dip = Dip(12.0);
/// The thickness of the band's bottom hairline.
const BORDER_WIDTH: f32 = 1.0;

/// Paints the bar's background and every item into `canvas`.
pub(super) fn paint(
    canvas: &mut dyn Canvas,
    items: &Rc<RefCell<Vec<Item>>>,
    hover: &Cell<Option<usize>>,
    active: &Cell<Option<usize>>,
    selected: &Cell<bool>,
    theme: Theme,
) {
    let bounds = canvas.bounds();
    let dpi = canvas.dpi();
    canvas.clear(theme.surface);
    canvas.draw_line(
        Point::new(bounds.left, bounds.bottom - 1),
        Point::new(bounds.right, bounds.bottom - 1),
        theme.border,
        BORDER_WIDTH,
    );

    let borrowed = items.borrow();
    let hot = |index| active.get() == Some(index) || hover.get() == Some(index);
    items::each_rect(&borrowed[..], bounds, dpi, |index, rect| {
        let item = &borrowed[index];
        match &item.kind {
            Kind::Label(text) => {
                let color = if item.enabled {
                    theme.text_secondary
                } else {
                    theme.text_disabled
                };
                let inset = PADDING.to_px(dpi).value();
                let text_rect = Rect::new(rect.left + inset, rect.top, rect.right, rect.bottom);
                canvas.draw_text(text, text_rect, &TextStyle::new(color, LABEL_SIZE).middle());
            }
            Kind::Spacer(_) => {}
            Kind::Icon(glyph) => {
                if active.get() == Some(index) {
                    canvas.fill_rounded_rect(rect, RADIUS, theme.pressed);
                } else if hot(index) {
                    canvas.fill_rounded_rect(rect, RADIUS, theme.hover);
                }
                let color = if item.enabled {
                    theme.text
                } else {
                    theme.text_disabled
                };
                draw_icon(canvas, *glyph, rect, color, dpi);
            }
            Kind::Toggle { glyph, checked } => {
                if active.get() == Some(index) {
                    canvas.fill_rounded_rect(rect, RADIUS, theme.pressed);
                } else if hot(index) {
                    canvas.fill_rounded_rect(rect, RADIUS, theme.hover);
                }
                let checked = *checked && item.enabled;
                if checked {
                    canvas.fill_rounded_rect(rect, RADIUS, theme.accent);
                }
                let color = if !item.enabled {
                    theme.text_disabled
                } else if checked {
                    theme.text_on_accent
                } else {
                    theme.text
                };
                draw_icon(canvas, *glyph, rect, color, dpi);
            }
            Kind::Slider { min, max, value } => {
                draw_slider(canvas, rect, theme, dpi, (*min, *max, *value), item.enabled);
            }
        }
    });

    if selected.get() {
        canvas.stroke_rect(bounds, theme.accent, 2.0);
    }
}

/// Paints a slider item's track and thumb. `range` is `(min, max, value)`.
fn draw_slider(
    canvas: &mut dyn Canvas,
    rect: Rect,
    theme: Theme,
    dpi: u32,
    range: (f64, f64, f64),
    enabled: bool,
) {
    let (min, max, value) = range;
    let track = TRACK.to_px(dpi).value().max(1);
    let thumb = THUMB.to_px(dpi).value().max(1);
    let middle = (rect.top + rect.bottom) / 2;
    let left = rect.left + thumb;
    let right = rect.right - thumb;
    let span = (right - left).max(1);
    let range = (max - min).max(f64::EPSILON);
    let fraction = ((value - min) / range).clamp(0.0, 1.0);
    let x = left + (span as f64 * fraction).round() as i32;
    let color = if enabled {
        theme.accent
    } else {
        theme.text_disabled
    };

    let full = Rect::new(left, middle - track / 2, right, middle - track / 2 + track);
    canvas.fill_rounded_rect(full, track as f32 / 2.0, theme.scrollbar_track);
    let filled = Rect::new(left, middle - track / 2, x, middle - track / 2 + track);
    canvas.fill_rounded_rect(filled, track as f32 / 2.0, color);
    canvas.fill_ellipse(Point::new(x, middle), thumb as f32, thumb as f32, color);
}
