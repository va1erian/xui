#![forbid(unsafe_code)]

//! The built-in [`Glyph`](super::Glyph) icon set, drawn with portable
//! [`Canvas`] primitives.
//!
//! The transport shapes (play, pause, stop, previous, next, repeat, shuffle)
//! are vectors rather than text glyphs: a fallback font has no Private Use
//! Area coverage for an icon font on every backend, and its metrics would not
//! line up with the button box anyway.

use crate::backend::{Canvas, TextStyle};
use crate::color::Color;
use crate::geometry::{Point, Rect};
use crate::units::Dip;

use super::Glyph;
use super::nav_glyph;

/// The design size of a drawn glyph box.
const GLYPH: Dip = Dip(16.0);
/// The design size of a text glyph.
const TEXT_SIZE: Dip = Dip(14.0);

/// Draws `glyph` centred in `rect` in `color`.
///
/// A [`Glyph::Text`] run goes through [`Canvas::draw_text`]; every other glyph
/// is composed from lines, ellipses, rectangles and polygons, so the icon set
/// needs no image dependency and no font.
pub(crate) fn draw_glyph(
    canvas: &mut dyn Canvas,
    glyph: Glyph,
    rect: Rect,
    color: Color,
    dpi: u32,
) {
    if let Glyph::Text(text) = glyph {
        let style = TextStyle::new(color, TEXT_SIZE).centered().middle();
        canvas.draw_text(text, rect, &style);
        return;
    }

    let size = GLYPH.to_px(dpi).value().max(2);
    let half = size / 2;
    let center_x = (rect.left + rect.right) / 2;
    let center_y = (rect.top + rect.bottom) / 2;
    let stroke = (dpi as f32 / 96.0 * 1.5).max(1.0);

    match glyph {
        Glyph::Menu => {
            for step in -1..=1 {
                let y = center_y + step * (size / 3);
                canvas.draw_line(
                    Point::new(center_x - half, y),
                    Point::new(center_x + half, y),
                    color,
                    stroke,
                );
            }
        }
        Glyph::Close => {
            let arm = half * 2 / 3;
            canvas.draw_line(
                Point::new(center_x - arm, center_y - arm),
                Point::new(center_x + arm, center_y + arm),
                color,
                stroke,
            );
            canvas.draw_line(
                Point::new(center_x - arm, center_y + arm),
                Point::new(center_x + arm, center_y - arm),
                color,
                stroke,
            );
        }
        Glyph::More => {
            let radius = (size / 8).max(1) as f32;
            for step in -1..=1 {
                canvas.fill_ellipse(
                    Point::new(center_x + step * size / 3, center_y),
                    radius,
                    radius,
                    color,
                );
            }
        }
        Glyph::Search => {
            let radius = (size / 3) as f32;
            let offset_x = center_x - size / 6;
            let offset_y = center_y - size / 6;
            canvas.stroke_ellipse(
                Point::new(offset_x, offset_y),
                radius,
                radius,
                color,
                stroke,
            );
            canvas.draw_line(
                Point::new(
                    offset_x + (radius * 0.7) as i32,
                    offset_y + (radius * 0.7) as i32,
                ),
                Point::new(center_x + half, center_y + half),
                color,
                stroke,
            );
        }
        Glyph::Star
        | Glyph::StarFilled
        | Glyph::Audio
        | Glyph::Album
        | Glyph::People
        | Glyph::Tag
        | Glyph::Folder
        | Glyph::History
        | Glyph::Monitor
        | Glyph::Settings => {
            nav_glyph::draw(
                canvas,
                glyph,
                nav_glyph::Frame {
                    cx: center_x,
                    cy: center_y,
                    half,
                    size,
                    stroke,
                },
                color,
            );
        }
        Glyph::Play => {
            // A right-pointing triangle nudged right, so its optical centre
            // lands on the button centre (the base carries more weight).
            let extent = size * 2 / 3;
            let shift = size / 10;
            let left = center_x - extent / 2 + shift;
            let tip = center_x + extent / 2 + shift;
            canvas.fill_polygon(
                &[
                    Point::new(left, center_y - extent / 2),
                    Point::new(tip, center_y),
                    Point::new(left, center_y + extent / 2),
                ],
                color,
            );
        }
        Glyph::Pause => {
            let bar = (size / 4).max(2);
            let gap = (size / 8).max(1);
            let top = center_y - size / 3;
            let bottom = center_y + size / 3;
            canvas.fill_rect(
                Rect::new(center_x - gap / 2 - bar, top, center_x - gap / 2, bottom),
                color,
            );
            canvas.fill_rect(
                Rect::new(center_x + gap / 2, top, center_x + gap / 2 + bar, bottom),
                color,
            );
        }
        Glyph::Stop => {
            let extent = size * 2 / 3;
            let stop = Rect::new(
                center_x - extent / 2,
                center_y - extent / 2,
                center_x + extent / 2,
                center_y + extent / 2,
            );
            canvas.fill_rounded_rect(stop, (extent / 8).max(1) as f32, color);
        }
        Glyph::Previous => {
            let extent = size * 3 / 8;
            let bar = (size / 6).max(2);
            let top = center_y - size / 3;
            let bottom = center_y + size / 3;
            let bar_left = center_x - extent;
            canvas.fill_rect(Rect::new(bar_left, top, bar_left + bar, bottom), color);
            canvas.fill_polygon(
                &[
                    Point::new(center_x + extent, top),
                    Point::new(center_x + extent, bottom),
                    Point::new(bar_left + bar + 1, center_y),
                ],
                color,
            );
        }
        Glyph::Next => {
            let extent = size * 3 / 8;
            let bar = (size / 6).max(2);
            let top = center_y - size / 3;
            let bottom = center_y + size / 3;
            let bar_right = center_x + extent;
            canvas.fill_rect(Rect::new(bar_right - bar, top, bar_right, bottom), color);
            canvas.fill_polygon(
                &[
                    Point::new(center_x - extent, top),
                    Point::new(center_x - extent, bottom),
                    Point::new(bar_right - bar - 1, center_y),
                ],
                color,
            );
        }
        Glyph::Repeat => {
            let half_w = size * 5 / 16;
            let half_h = size * 3 / 16;
            let head = (size / 3).max(3);
            let (left, right) = (center_x - half_w, center_x + half_w);
            let (top, bottom) = (center_y - half_h, center_y + half_h);
            // An open rectangular loop with an arrow head at the top-right and
            // another at the bottom-left, so the direction reads as repeat.
            canvas.draw_line(
                Point::new(left, top),
                Point::new(right - head, top),
                color,
                stroke,
            );
            canvas.draw_line(
                Point::new(right, top),
                Point::new(right, bottom),
                color,
                stroke,
            );
            canvas.draw_line(
                Point::new(right, bottom),
                Point::new(left + head, bottom),
                color,
                stroke,
            );
            canvas.draw_line(
                Point::new(left, bottom),
                Point::new(left, top),
                color,
                stroke,
            );
            arrow_head(canvas, Point::new(right, top), (1.0, 0.0), head, color);
            arrow_head(canvas, Point::new(left, bottom), (-1.0, 0.0), head, color);
        }
        Glyph::Shuffle => {
            let extent = size * 5 / 16;
            let head = (size / 3).max(3);
            let (left, right) = (center_x - extent, center_x + extent);
            let (top, bottom) = (center_y - extent, center_y + extent);
            canvas.draw_line(
                Point::new(left, top),
                Point::new(right, bottom),
                color,
                stroke,
            );
            canvas.draw_line(
                Point::new(left, bottom),
                Point::new(right, top),
                color,
                stroke,
            );
            // The heads follow each diagonal, so the strokes do not bend.
            let down = (2.0 * extent as f32, 2.0 * extent as f32);
            arrow_head(canvas, Point::new(right, bottom), down, head, color);
            arrow_head(
                canvas,
                Point::new(right, top),
                (down.0, -down.1),
                head,
                color,
            );
        }
        Glyph::Text(_) => {}
    }
}

/// Fills a triangular arrow head at `tip`, `head` pixels long, pointing along
/// the (unnormalised) direction `dir`.
fn arrow_head(canvas: &mut dyn Canvas, tip: Point, dir: (f32, f32), head: i32, color: Color) {
    let head = head.max(2) as f32;
    let length = (dir.0 * dir.0 + dir.1 * dir.1).sqrt().max(f32::EPSILON);
    let (ux, uy) = (dir.0 / length, dir.1 / length);
    let base = (tip.x as f32 - ux * head, tip.y as f32 - uy * head);
    let half = (head / 2.0).max(1.0);
    let (px, py) = (-uy * half, ux * half);
    canvas.fill_polygon(
        &[
            Point::new((base.0 + px).round() as i32, (base.1 + py).round() as i32),
            Point::new((base.0 - px).round() as i32, (base.1 - py).round() as i32),
            Point::new(tip.x, tip.y),
        ],
        color,
    );
}
