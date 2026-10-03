#![forbid(unsafe_code)]

//! The icon view's painter: the viewport background, then only the visible
//! tiles. Each tile is an icon on the left and up to three lines of text on the
//! right, drawn from the theme's semantic tokens in every state.

use super::layout;
use super::metrics::Metrics;
use super::state::State;
use crate::backend::{Canvas, Dash, Rgba, Stroke, TextStyle};
use crate::color::Color;
use crate::geometry::{Point, Rect};
use crate::icon::draw_icon;
use crate::theme::Theme;
use crate::theme::look::backdrop;

/// Draws `state` into `canvas`. Only the visible tiles are touched, so a large
/// model costs the same as a small one.
pub(crate) fn paint(canvas: &mut dyn Canvas, state: &State, theme: &Theme, outline: bool) {
    let bounds = canvas.bounds();
    backdrop(canvas, theme.background);
    if bounds.is_empty() {
        if outline {
            canvas.stroke_rect(bounds, theme.accent, 2.0);
        }
        return;
    }

    let dpi = canvas.dpi();
    let metrics = state.metrics(dpi);
    let viewport = state.viewport(bounds, dpi);
    let content = Rect::new(
        bounds.left,
        bounds.top,
        bounds.left + viewport.width,
        bounds.bottom,
    );
    let range = layout::visible_range(
        state.offset,
        viewport.height,
        viewport.columns,
        metrics,
        state.len(),
    );

    canvas.push_clip(content);
    for index in range {
        let cell = layout::tile_rect(index, viewport.columns, metrics);
        let tile = cell.offset(content.left, content.top - state.offset);
        paint_tile(canvas, state, theme, tile, index, metrics, dpi);
    }
    canvas.pop_clip();

    if outline {
        canvas.stroke_rect(bounds, theme.accent, 2.0);
    }
}

/// Draws one tile: its background, icon and text lines, then the focus ring.
fn paint_tile(
    canvas: &mut dyn Canvas,
    state: &State,
    theme: &Theme,
    tile: Rect,
    index: usize,
    metrics: Metrics,
    dpi: u32,
) {
    let selected = state.selected.contains(&index);
    let focused = state.enabled && state.has_focus && state.focused == Some(index);
    let icon_rect = metrics.icon_rect(tile);
    let text_rect = metrics.text_rect(tile);

    let fill = if !state.enabled {
        None
    } else if selected {
        Some(if state.has_focus {
            theme.accent
        } else {
            theme.selection_unfocused
        })
    } else if state.hover == Some(index) {
        Some(theme.hover)
    } else {
        None
    };
    if let Some(fill) = fill {
        canvas.fill_rect(text_rect, fill);
    }

    let (primary, secondary, icon_color) = colors(state, theme, selected);
    canvas.push_clip(tile);
    if !state.model.paint_icon(index, canvas, icon_rect, theme, dpi)
        && let Some(icon) = state.model.icon(index)
    {
        draw_icon(canvas, icon, icon_rect, icon_color, dpi);
    }
    canvas.pop_clip();

    for line in 0..metrics.lines {
        let Some(rect) = metrics.line_rect(text_rect, line) else {
            break;
        };
        let Some(text) = state.model.line(index, line) else {
            continue;
        };
        if text.is_empty() {
            continue;
        }
        let color = if line == 0 { primary } else { secondary };
        draw_line(canvas, text, rect, color, metrics.text_size);
    }

    if focused {
        let ring = if selected {
            theme.text_on_accent
        } else {
            theme.text
        };
        draw_focus_ring(canvas, text_rect, ring);
    }
}

/// The text and icon colours of a tile in its current state. Disabled wins, then
/// an active selection (drawn on the accent fill), then an inactive selection,
/// then the ordinary text colours.
fn colors(state: &State, theme: &Theme, selected: bool) -> (Color, Color, Color) {
    if !state.enabled {
        return (
            theme.text_disabled,
            theme.text_disabled,
            theme.text_disabled,
        );
    }
    if selected && state.has_focus {
        return (theme.text_on_accent, theme.text_on_accent, theme.accent);
    }
    (theme.text, theme.text_secondary, theme.text)
}

/// Draws one line of `text`, end-ellipsised into `rect` and clipped to it so a
/// truncation that slightly overshoots never bleeds into the next tile.
fn draw_line(
    canvas: &mut dyn Canvas,
    text: &str,
    rect: Rect,
    color: Color,
    size: crate::units::Dip,
) {
    canvas.push_clip(rect);
    let fitted = super::super::ellipsis::truncate(text, rect.width(), &mut |candidate| {
        canvas
            .measure_text(candidate, &TextStyle::new(color, size))
            .width
    });
    let style = TextStyle::new(color, size).middle();
    canvas.draw_text(&fitted, rect, &style);
    canvas.pop_clip();
}

/// Draws a dotted focus rectangle just outside `rect`.
fn draw_focus_ring(canvas: &mut dyn Canvas, rect: Rect, color: Color) {
    let ring = Rect::new(rect.left - 1, rect.top - 1, rect.right + 1, rect.bottom + 1);
    if ring.is_empty() {
        return;
    }
    let stroke = Stroke::new(1.0).dash(Dash::Dotted);
    let rgba = Rgba::from(color);
    let corners = [
        Point::new(ring.left, ring.top),
        Point::new(ring.right - 1, ring.top),
        Point::new(ring.right - 1, ring.bottom - 1),
        Point::new(ring.left, ring.bottom - 1),
    ];
    for index in 0..4 {
        let from = corners[index];
        let to = corners[(index + 1) % 4];
        canvas.draw_line_stroked(from, to, rgba, &stroke);
    }
}
