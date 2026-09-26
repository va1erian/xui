#![forbid(unsafe_code)]

//! The list view's painter: the header row, then only the visible body rows.

use super::state::{HEADER, PADDING, ROW, State, TEXT_SIZE, column_spans, column_widths};
use crate::backend::{Canvas, TextAlign, TextStyle};
use crate::geometry::{Point, Rect};
use crate::theme::Theme;

/// Draws `state` into `canvas`. Only the visible rows are touched, so a large
/// model costs the same as a small one.
pub(crate) fn paint(canvas: &mut dyn Canvas, state: &State, theme: &Theme, outline: bool) {
    let bounds = canvas.bounds();
    let dpi = canvas.dpi();
    canvas.clear(theme.background);

    let row_px = ROW.to_px(dpi).value().max(1);
    let body = if state.has_header() {
        let height = HEADER.to_px(dpi).value();
        let header = Rect::new(bounds.left, bounds.top, bounds.right, bounds.top + height);
        paint_header(canvas, state, theme, header, dpi);
        Rect::new(header.left, header.bottom, bounds.right, bounds.bottom)
    } else {
        bounds
    };

    if body.is_empty() {
        if outline {
            canvas.stroke_rect(bounds, theme.accent, 2.0);
        }
        return;
    }

    let widths = column_widths(dpi, bounds.width(), &state.columns);
    let spans = column_spans(&widths);
    let visible = (body.height() / row_px) as usize;
    canvas.push_clip(body);
    let mut top = body.top;
    for slot in 0..visible {
        let row = state.offset + slot;
        if row >= state.len() {
            break;
        }
        let rect = Rect::new(bounds.left, top, bounds.right, top + row_px);
        paint_row(canvas, state, theme, rect, row, &spans, dpi);
        top += row_px;
    }
    canvas.pop_clip();

    if outline {
        canvas.stroke_rect(bounds, theme.accent, 2.0);
    }
}

fn paint_header(canvas: &mut dyn Canvas, state: &State, theme: &Theme, rect: Rect, dpi: u32) {
    canvas.fill_rect(rect, theme.surface);
    canvas.draw_line(
        Point::new(rect.left, rect.bottom - 1),
        Point::new(rect.right, rect.bottom - 1),
        theme.border,
        1.0,
    );
    let widths = column_widths(dpi, rect.width(), &state.columns);
    let spans = column_spans(&widths);
    let pad = PADDING.to_px(dpi).value();
    for (index, column) in state.columns.iter().enumerate() {
        let Some(&(left, right)) = spans.get(index) else {
            continue;
        };
        if index > 0 {
            canvas.draw_line(
                Point::new(rect.left + left, rect.top),
                Point::new(rect.left + left, rect.bottom),
                theme.border,
                1.0,
            );
        }
        let cell = Rect::new(rect.left + left, rect.top, rect.left + right, rect.bottom);
        let text = Rect::new(cell.left + pad, cell.top, cell.right - pad, cell.bottom);
        let style = aligned(TextStyle::new(theme.text, TEXT_SIZE).middle(), column);
        // A long title must not run into the next column: clip the cell. The
        // clip is cheap now that the backend draws a whole node in one frame.
        canvas.push_clip(cell);
        canvas.draw_text(&column.title, text, &style);
        canvas.pop_clip();
        if state.sort.is_some_and(|(sorted, _)| sorted == index) {
            paint_sort_arrow(canvas, state.sort, cell, theme, dpi);
        }
    }
}

/// Draws the ascending/descending triangle at the right edge of a sorted
/// header cell.
fn paint_sort_arrow(
    canvas: &mut dyn Canvas,
    sort: Option<(usize, super::model::SortDirection)>,
    cell: Rect,
    theme: &Theme,
    dpi: u32,
) {
    use super::model::SortDirection;
    let half = (PADDING.to_px(dpi).value() / 2).max(2);
    let pad = PADDING.to_px(dpi).value();
    let cx = cell.right - pad - half;
    let cy = (cell.top + cell.bottom) / 2;
    let points = match sort {
        Some((_, SortDirection::Ascending)) => [
            Point::new(cx - half, cy + half),
            Point::new(cx + half, cy + half),
            Point::new(cx, cy - half),
        ],
        _ => [
            Point::new(cx - half, cy - half),
            Point::new(cx + half, cy - half),
            Point::new(cx, cy + half),
        ],
    };
    canvas.fill_polygon(&points, theme.text);
}

fn paint_row(
    canvas: &mut dyn Canvas,
    state: &State,
    theme: &Theme,
    rect: Rect,
    row: usize,
    spans: &[(i32, i32)],
    dpi: u32,
) {
    let selected = state.selected.contains(&row);
    if selected {
        canvas.fill_rect(rect, theme.accent);
    } else if state.hover == Some(row) {
        canvas.fill_rect(rect, theme.hover);
    }
    let color = match (state.enabled, selected) {
        (false, _) => theme.text_disabled,
        (true, true) => theme.text_on_accent,
        (true, false) => theme.text,
    };
    let pad = PADDING.to_px(dpi).value();

    if state.columns.is_empty() {
        if row > state.offset {
            canvas.draw_line(
                Point::new(rect.left, rect.top),
                Point::new(rect.right, rect.top),
                theme.border,
                1.0,
            );
        }
        let text = Rect::new(rect.left + pad, rect.top, rect.right - pad, rect.bottom);
        let style = TextStyle::new(color, TEXT_SIZE).middle();
        canvas.push_clip(rect);
        canvas.draw_text(state.rows.cell(row, 0).unwrap_or(""), text, &style);
        canvas.pop_clip();
        return;
    }

    for (index, column) in state.columns.iter().enumerate() {
        let Some(&(left, right)) = spans.get(index) else {
            break;
        };
        if index > 0 {
            canvas.draw_line(
                Point::new(rect.left + left, rect.top),
                Point::new(rect.left + left, rect.bottom),
                theme.border,
                1.0,
            );
        }
        let cell = Rect::new(rect.left + left, rect.top, rect.left + right, rect.bottom);
        let text = Rect::new(cell.left + pad, cell.top, cell.right - pad, cell.bottom);
        let style = aligned(TextStyle::new(color, TEXT_SIZE).middle(), column);
        // Clip the cell so a long value cannot overlap the next column.
        canvas.push_clip(cell);
        canvas.draw_text(state.rows.cell(row, index).unwrap_or(""), text, &style);
        canvas.pop_clip();
    }
}

/// Applies a column's alignment to `style`.
fn aligned(mut style: TextStyle, column: &super::model::Column) -> TextStyle {
    if column.align_right {
        style.align = TextAlign::End;
    } else if column.centered {
        style.align = TextAlign::Center;
    }
    style
}
