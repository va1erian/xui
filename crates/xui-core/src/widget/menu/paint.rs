#![forbid(unsafe_code)]

//! Painting the [`Menu`](super::Menu) bar and its popups from theme tokens.
//!
//! Everything draws in the canvas's own coordinate frame, so the same code
//! paints a native node (whose origin is `0, 0`) and the composited canvas
//! backend (whose frame is window-absolute).

use crate::backend::{Canvas, TextStyle};
use crate::geometry::{Point, Rect};
use crate::icon::draw_icon;
use crate::theme::Theme;
use crate::widget::popup;

use super::View;
use super::layout;
use super::model::{self, Kind, Node};

/// Paints the bar: title cells, the open/hover highlight and the mnemonics.
pub(super) fn bar(
    nodes: &[Node],
    view: &View,
    canvas: &mut dyn Canvas,
    theme: Theme,
    selected: bool,
) {
    let bounds = canvas.bounds();
    canvas.clear(theme.surface);
    let dpi = canvas.dpi();
    let origin = Point::new(bounds.left, bounds.top);
    let pad = layout::BAR_PAD.to_px(dpi).value();
    layout::each_title(nodes, origin, bounds.height(), dpi, |index, rect| {
        let node = &nodes[index];
        if view.bar_open == Some(index) {
            canvas.fill_rect(rect, theme.selection);
        } else if view.bar_hover == Some(index) {
            canvas.fill_rect(rect, theme.hover);
        }
        let color = pick(node.enabled, theme.text, theme.text_disabled);
        let style = TextStyle::new(color, layout::TEXT_SIZE).middle();
        canvas.draw_text(&node.text, set_left(rect, pad), &style);
        underline(canvas, node, rect, color, pad, dpi);
    });
    if selected {
        canvas.stroke_rect(bounds, theme.accent, 2.0);
    }
}

/// Paints one popup level: background, border, rows and marks.
pub(super) fn popup(
    nodes: &[Node],
    view: &View,
    depth: usize,
    canvas: &mut dyn Canvas,
    theme: Theme,
) {
    let bounds = canvas.bounds();
    popup::paint(canvas, theme, theme.raised);
    let Some(level) = view.levels.get(depth) else {
        return;
    };
    let dpi = canvas.dpi();
    let origin = Point::new(bounds.left, bounds.top);
    let width = bounds.width();
    let pad = layout::PAD.to_px(dpi).value();
    let _ = model::with_entries(nodes, &level.path, |entries| {
        let (mark_column, icon_column) = layout::columns(entries, dpi);
        let mark = mark_column + icon_column;
        layout::each_row(entries, origin, width, dpi, |index, rect| {
            let node = &entries[index];
            if node.kind == Kind::Separator {
                let y = rect.top + rect.height() / 2;
                canvas.draw_line(
                    Point::new(rect.left + pad, y),
                    Point::new(rect.right - pad, y),
                    theme.border,
                    1.0,
                );
                return;
            }
            if level.hover == Some(index) {
                canvas.fill_rect(rect, theme.hover);
            }
            let color = pick(node.enabled, theme.text, theme.text_disabled);
            draw_mark(canvas, node, rect, color, dpi);
            draw_icon_column(canvas, node, rect, mark_column, icon_column, color, dpi);
            let style = TextStyle::new(color, layout::TEXT_SIZE).middle();
            canvas.draw_text(&node.text, set_left(rect, mark), &style);
            underline(canvas, node, rect, color, mark, dpi);
            if node.kind == Kind::Submenu {
                draw_arrow(canvas, rect, color, dpi);
            }
        });
    });
}

/// A rectangle with its left edge moved right by `pad`.
fn set_left(rect: Rect, pad: i32) -> Rect {
    Rect::new(rect.left + pad, rect.top, rect.right, rect.bottom)
}

/// Picks `yes` when `condition` holds, else `no`.
fn pick<T>(condition: bool, yes: T, no: T) -> T {
    if condition { yes } else { no }
}

/// Draws the underline of a label's mnemonic character, estimated from the
/// character count (the portable canvas does not measure text).
fn underline(
    canvas: &mut dyn Canvas,
    node: &Node,
    rect: Rect,
    color: crate::color::Color,
    pad: i32,
    dpi: u32,
) {
    let Some(index) = node.mnemonic else {
        return;
    };
    let char_width = layout::CHAR.to_px(dpi).value().max(1);
    let x = rect.left + pad + char_width * index as i32;
    let y = rect.top + rect.height() * 3 / 4;
    canvas.draw_line(Point::new(x, y), Point::new(x + char_width, y), color, 1.0);
}

/// Draws an entry's leading icon, centred in the icon column that starts
/// `offset` past the row's left edge. Nothing is drawn for an entry without an
/// icon or in a popup with no icon column.
fn draw_icon_column(
    canvas: &mut dyn Canvas,
    node: &Node,
    rect: Rect,
    offset: i32,
    width: i32,
    color: crate::color::Color,
    dpi: u32,
) {
    let Some(icon) = node.icon else {
        return;
    };
    let side = layout::ICON.to_px(dpi).value().min(width).max(1);
    let left = rect.left + offset + (width - side) / 2;
    let top = rect.top + (rect.height() - side) / 2;
    draw_icon(
        canvas,
        icon,
        Rect::new(left, top, left + side, top + side),
        color,
        dpi,
    );
}

/// Draws a check mark or radio dot in the mark column.
fn draw_mark(
    canvas: &mut dyn Canvas,
    node: &Node,
    rect: Rect,
    color: crate::color::Color,
    dpi: u32,
) {
    let mark = layout::MARK.to_px(dpi).value();
    let size = layout::BOX.to_px(dpi).value().max(4) as f32;
    let left = (rect.left + (mark - size as i32) / 2) as f32;
    let top = (rect.top + (rect.height() - size as i32) / 2) as f32;
    match node.kind {
        Kind::Check if node.checked => {
            canvas.draw_line(
                Point::new((left + size * 0.20) as i32, (top + size * 0.50) as i32),
                Point::new((left + size * 0.42) as i32, (top + size * 0.72) as i32),
                color,
                1.8,
            );
            canvas.draw_line(
                Point::new((left + size * 0.42) as i32, (top + size * 0.72) as i32),
                Point::new((left + size * 0.80) as i32, (top + size * 0.25) as i32),
                color,
                1.8,
            );
        }
        Kind::Radio if node.checked => {
            let radius = size / 2.0;
            let center = Point::new(rect.left + mark / 2, rect.top + rect.height() / 2);
            canvas.fill_ellipse(center, radius, radius, color);
        }
        _ => {}
    }
}

/// Draws the submenu chevron at a row's right edge.
fn draw_arrow(canvas: &mut dyn Canvas, rect: Rect, color: crate::color::Color, dpi: u32) {
    let size = layout::GLYPH.to_px(dpi).value().max(3);
    let pad = layout::PAD.to_px(dpi).value();
    let x = rect.right - pad - size;
    let y = rect.top + rect.height() / 2;
    canvas.draw_line(Point::new(x, y - size), Point::new(x + size, y), color, 1.5);
    canvas.draw_line(Point::new(x + size, y), Point::new(x, y + size), color, 1.5);
}
