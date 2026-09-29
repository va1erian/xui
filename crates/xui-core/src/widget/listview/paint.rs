#![forbid(unsafe_code)]

//! The list view's painter: the header row, then only the visible body rows.

use super::super::ellipsis;
use super::state::{
    ICON, ICON_GAP, PADDING, ROW, State, TEXT_SIZE, column_spans, column_widths, header_px,
};
use crate::backend::{Canvas, TextAlign, TextStyle};
use crate::color::Color;
use crate::geometry::{Point, Rect};
use crate::icon::draw_icon;
use crate::theme::Theme;
use crate::widget::scrollbar;

/// Draws `text` truncated with an end-ellipsis when it is wider than `rect`,
/// clipping the cell either way so a truncation that still slightly overshoots
/// (the measurer and the rasterizer can disagree by a pixel or two) never
/// bleeds into the next column.
fn draw_cell_text(canvas: &mut dyn Canvas, text: &str, cell: Rect, rect: Rect, style: &TextStyle) {
    canvas.push_clip(cell);
    let fitted = ellipsis::truncate(text, rect.width(), &mut |candidate| {
        canvas.measure_text(candidate, style).width
    });
    canvas.draw_text(&fitted, rect, style);
    canvas.pop_clip();
}

/// Draws `state` into `canvas`. Only the visible rows are touched, so a large
/// model costs the same as a small one.
pub(crate) fn paint(canvas: &mut dyn Canvas, state: &State, theme: &Theme, outline: bool) {
    let bounds = canvas.bounds();
    let dpi = canvas.dpi();
    canvas.clear(theme.background);

    let row_px = ROW.to_px(dpi).value().max(1);
    let header_h = header_px(state.has_header(), dpi);
    // Reserve the bar's width so the last column does not run under it.
    let overflows = state.len() as i32 * row_px > (bounds.height() - header_h).max(0);
    let reserve = if overflows {
        scrollbar::THICKNESS.to_px(dpi).value()
    } else {
        0
    };
    let content = Rect::new(
        bounds.left,
        bounds.top,
        bounds.right - reserve,
        bounds.bottom,
    );
    let body = if state.has_header() {
        let header = Rect::new(
            content.left,
            content.top,
            content.right,
            content.top + header_h,
        );
        paint_header(canvas, state, theme, header, dpi);
        Rect::new(header.left, header.bottom, content.right, content.bottom)
    } else {
        content
    };

    if body.is_empty() {
        if outline {
            canvas.stroke_rect(bounds, theme.accent, 2.0);
        }
        return;
    }

    let widths = column_widths(dpi, body.width(), &state.columns);
    let spans = column_spans(&widths);
    let visible = (body.height() / row_px) as usize;
    canvas.push_clip(body);
    let mut top = body.top;
    for slot in 0..visible {
        let row = state.offset + slot;
        if row >= state.len() {
            break;
        }
        let rect = Rect::new(body.left, top, body.right, top + row_px);
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
        let sorted = state.sort.is_some_and(|(sorted, _)| sorted == index);
        // The sorted column reserves room for its arrow so a long title is
        // ellipsized before it, rather than drawn underneath it.
        let text_right = if sorted {
            cell.right - pad - sort_arrow_width(dpi)
        } else {
            cell.right - pad
        };
        let text = Rect::new(cell.left + pad, cell.top, text_right, cell.bottom);
        let style = aligned(TextStyle::new(theme.text, TEXT_SIZE).middle(), column);
        // A long title must not run into the next column: ellipsize it, and
        // clip the cell too in case the measurer and rasterizer disagree.
        draw_cell_text(canvas, &column.title, cell, text, &style);
        if sorted {
            paint_sort_arrow(canvas, state.sort, cell, theme, dpi);
        }
    }
}

/// Half the arrow's width; the full triangle spans `2 * half`, plus the
/// padding gap the caller reserves before the title text.
fn sort_arrow_half(dpi: u32) -> i32 {
    (PADDING.to_px(dpi).value() / 2).max(2)
}

/// The width a sorted header cell must reserve, past its own trailing
/// padding, so the title text does not draw underneath the arrow.
fn sort_arrow_width(dpi: u32) -> i32 {
    sort_arrow_half(dpi) * 2
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
    let half = sort_arrow_half(dpi);
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
        let inset = paint_row_icon(canvas, state, row, rect, color, dpi);
        let text = Rect::new(
            rect.left + pad + inset,
            rect.top,
            rect.right - pad,
            rect.bottom,
        );
        let style = TextStyle::new(color, TEXT_SIZE).middle();
        draw_cell_text(
            canvas,
            state.rows.cell(row, 0).unwrap_or(""),
            rect,
            text,
            &style,
        );
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
        // The row's icon leads the first column only.
        let inset = if index == 0 {
            paint_row_icon(canvas, state, row, cell, color, dpi)
        } else {
            0
        };
        let text = Rect::new(
            cell.left + pad + inset,
            cell.top,
            cell.right - pad,
            cell.bottom,
        );
        let style = aligned(TextStyle::new(color, TEXT_SIZE).middle(), column);
        // A long value must not run into the next column: ellipsize it, and
        // clip the cell too in case the measurer and rasterizer disagree.
        draw_cell_text(
            canvas,
            state.rows.cell(row, index).unwrap_or(""),
            cell,
            text,
            &style,
        );
    }
}

/// Draws `row`'s leading icon, if the model gives it one, at the left of
/// `cell`, and returns how far the text after it must move right (`0` for a
/// row without an icon, so those keep their layout exactly).
fn paint_row_icon(
    canvas: &mut dyn Canvas,
    state: &State,
    row: usize,
    cell: Rect,
    color: Color,
    dpi: u32,
) -> i32 {
    let Some(icon) = state.rows.icon(row) else {
        return 0;
    };
    let side = ICON.to_px(dpi).value();
    let left = cell.left + PADDING.to_px(dpi).value();
    let top = cell.top + (cell.height() - side) / 2;
    canvas.push_clip(cell);
    draw_icon(
        canvas,
        icon,
        Rect::new(left, top, left + side, top + side),
        color,
        dpi,
    );
    canvas.pop_clip();
    side + ICON_GAP.to_px(dpi).value()
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
