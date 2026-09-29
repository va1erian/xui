#![forbid(unsafe_code)]

//! Painting for [`Edit`](super::Edit): the field, its cue, the selection
//! highlight, the text (scrolled so the caret stays visible) and the caret.
//!
//! Coordinates are device pixels; [`Canvas::bounds`] is the field's rectangle
//! in the window. Text width comes from the canvas itself, and the caret and
//! selection x come from [`super::geometry`], the same arithmetic hit-testing
//! is the inverse of, at the canvas's DPI.

use std::cell::{Cell, RefCell};

use super::TEXT_SIZE;
use super::geometry::{padding, text_x};
use super::model::EditModel;
use crate::backend::{Canvas, TextStyle};
use crate::geometry::{Point, Rect};
use crate::theme::Theme;

/// The live state a paint reads. All handles are shared with the widget, so a
/// repaint sees the latest text, caret and selection.
pub(super) struct PaintState<'a> {
    /// The edit model (text, caret, selection).
    pub model: &'a RefCell<EditModel>,
    /// The cue banner shown while the text is empty.
    pub cue: &'a RefCell<String>,
    /// The horizontal scroll offset in pixels, recomputed so the caret stays
    /// visible and kept between paints.
    pub scroll: &'a Cell<i32>,
    /// Whether the field has the keyboard focus.
    pub focused: &'a Cell<bool>,
    /// Whether a form editor has selected the field, so it draws an outline.
    pub selected: &'a Cell<bool>,
}

/// Paints the field described by `state`.
pub(super) fn paint(canvas: &mut dyn Canvas, theme: &Theme, state: &PaintState) {
    let bounds = canvas.bounds();
    canvas.clear(theme.input_background);
    let border = if state.focused.get() {
        theme.border_focused
    } else {
        theme.input_border
    };
    canvas.stroke_rect(bounds, border, 1.0);

    let dpi = canvas.dpi();
    let pad = padding(dpi);
    let inner = bounds.shrink(pad);
    let model = state.model.borrow();
    let value = model.text();
    let style = TextStyle::new(theme.text, TEXT_SIZE).middle();

    if value.is_empty() {
        if inner.width() > 0 && inner.height() > 0 {
            let cue = state.cue.borrow();
            canvas.draw_text(
                &cue,
                inner,
                &TextStyle::new(theme.text_disabled, TEXT_SIZE).middle(),
            );
        }
        if state.focused.get() {
            draw_caret(canvas, inner.left, inner.top, inner.bottom, theme.text);
        }
    } else if inner.width() > 0 && inner.height() > 0 {
        paint_text(
            canvas,
            theme,
            state,
            &model,
            &style,
            bounds.left,
            inner,
            dpi,
        );
    }

    if state.selected.get() {
        canvas.stroke_rect(bounds, theme.accent, 2.0);
    }
}

/// Paints the text, its selection and the caret, scrolled so the caret is
/// visible.
#[allow(clippy::too_many_arguments)]
fn paint_text(
    canvas: &mut dyn Canvas,
    theme: &Theme,
    state: &PaintState,
    model: &EditModel,
    style: &TextStyle,
    left: i32,
    inner: Rect,
    dpi: u32,
) {
    let value = model.text();
    let caret = model.caret().min(value.chars().count());
    let (selection_start, selection_end) = model.selection();
    let view = inner.width();
    let full_width = canvas.measure_text(value, style).width;
    let caret_width = width_at(canvas, value, caret, style);
    let max_scroll = (full_width - view).max(0);

    // Keep the caret in view: scroll right when it passes the right edge, left
    // when it passes the left one, then clamp to the text's own extent.
    let mut scroll = state.scroll.get().clamp(0, max_scroll);
    if caret_width - scroll > view {
        scroll = caret_width - view;
    } else if caret_width - scroll < 0 {
        scroll = caret_width;
    }
    state.scroll.set(scroll.clamp(0, max_scroll));

    canvas.push_clip(inner);
    canvas.save();
    canvas.set_translation(-(scroll as f32), 0.0);
    if selection_end > selection_start {
        let from = text_x(
            left,
            width_at(canvas, value, selection_start, style),
            dpi,
            scroll,
        );
        let to = text_x(
            left,
            width_at(canvas, value, selection_end, style),
            dpi,
            scroll,
        );
        let highlight = if state.focused.get() {
            theme.selection
        } else {
            theme.selection_unfocused
        };
        canvas.fill_rect(Rect::new(from, inner.top, to, inner.bottom), highlight);
    }
    canvas.draw_text(value, inner, style);
    if state.focused.get() {
        let x = text_x(left, caret_width, dpi, scroll);
        draw_caret(canvas, x, inner.top, inner.bottom, theme.text);
    }
    canvas.restore();
    canvas.pop_clip();
}

/// Draws the caret as a one-pixel vertical line.
fn draw_caret(canvas: &mut dyn Canvas, x: i32, top: i32, bottom: i32, color: crate::color::Color) {
    canvas.draw_line(Point::new(x, top), Point::new(x, bottom), color, 1.0);
}

/// The width in pixels of the first `chars` characters of `text`.
fn width_at(canvas: &dyn Canvas, text: &str, chars: usize, style: &TextStyle) -> i32 {
    let byte = char_boundary(text, chars);
    canvas.measure_text(&text[..byte], style).width
}

/// The byte offset of the `chars`-th character, or the text's length.
fn char_boundary(text: &str, chars: usize) -> usize {
    text.char_indices()
        .nth(chars)
        .map_or(text.len(), |(at, _)| at)
}
