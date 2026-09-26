#![forbid(unsafe_code)]

//! A small portable vector icon set, drawn with the existing [`Canvas`]
//! primitives so it needs no image dependency and works on every backend.

use crate::backend::Canvas;
use crate::color::Color;
use crate::geometry::{Point, Rect};

/// A button-appropriate icon.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Icon {
    /// A plus sign (add / new).
    Plus,
    /// A minus sign (remove).
    Minus,
    /// A cross (close / dismiss).
    Close,
    /// A tick (confirm / done).
    Check,
    /// A downward chevron (expand).
    ChevronDown,
    /// An upward chevron (collapse).
    ChevronUp,
    /// A magnifier (search).
    Search,
    /// Three dots (more actions).
    More,
}

/// Draws `icon` centred in `rect` in `color`, with a 1.5px stroke scaled to
/// `dpi`.
pub fn draw_icon(canvas: &mut dyn Canvas, icon: Icon, rect: Rect, color: Color, dpi: u32) {
    let size = rect.width().min(rect.height());
    if size < 3 {
        return;
    }
    let cx = rect.left + rect.width() / 2;
    let cy = rect.top + rect.height() / 2;
    // The glyph is inset from the box so a stroked line is never clipped.
    let half = (size / 2 - 1).max(1);
    let stroke = (dpi as f32 / 96.0 * 1.5).max(1.0);
    let line = |canvas: &mut dyn Canvas, from: Point, to: Point| {
        canvas.draw_line(from, to, color, stroke);
    };

    match icon {
        Icon::Plus => {
            line(canvas, Point::new(cx - half, cy), Point::new(cx + half, cy));
            line(canvas, Point::new(cx, cy - half), Point::new(cx, cy + half));
        }
        Icon::Minus => {
            line(canvas, Point::new(cx - half, cy), Point::new(cx + half, cy));
        }
        Icon::Close => {
            let arm = half * 2 / 3;
            line(
                canvas,
                Point::new(cx - arm, cy - arm),
                Point::new(cx + arm, cy + arm),
            );
            line(
                canvas,
                Point::new(cx - arm, cy + arm),
                Point::new(cx + arm, cy - arm),
            );
        }
        Icon::Check => {
            line(
                canvas,
                Point::new(cx - half, cy),
                Point::new(cx - half / 3, cy + half * 2 / 3),
            );
            line(
                canvas,
                Point::new(cx - half / 3, cy + half * 2 / 3),
                Point::new(cx + half, cy - half * 2 / 3),
            );
        }
        Icon::ChevronDown => {
            line(
                canvas,
                Point::new(cx - half, cy - half / 3),
                Point::new(cx, cy + half / 3),
            );
            line(
                canvas,
                Point::new(cx, cy + half / 3),
                Point::new(cx + half, cy - half / 3),
            );
        }
        Icon::ChevronUp => {
            line(
                canvas,
                Point::new(cx - half, cy + half / 3),
                Point::new(cx, cy - half / 3),
            );
            line(
                canvas,
                Point::new(cx, cy - half / 3),
                Point::new(cx + half, cy + half / 3),
            );
        }
        Icon::Search => {
            let radius = (half * 2 / 3).max(1) as f32;
            let offset_x = cx - half / 3;
            let offset_y = cy - half / 3;
            canvas.stroke_ellipse(
                Point::new(offset_x, offset_y),
                radius,
                radius,
                color,
                stroke,
            );
            line(
                canvas,
                Point::new(
                    offset_x + (radius * 0.7) as i32,
                    offset_y + (radius * 0.7) as i32,
                ),
                Point::new(cx + half, cy + half),
            );
        }
        Icon::More => {
            let radius = (size / 8).max(1) as f32;
            for step in -1..=1 {
                canvas.fill_ellipse(Point::new(cx + step * size / 3, cy), radius, radius, color);
            }
        }
    }
}
