#![forbid(unsafe_code)]

//! The navigation [`Glyph`](super::Glyph) shapes: audio, album, people, tag,
//! folder, star, history, monitor and settings, drawn with portable
//! [`Canvas`] primitives.
//!
//! Split out of [`icon`](super::icon) so the transport set and this one each
//! stay small.

use crate::backend::Canvas;
use crate::color::Color;
use crate::geometry::{Point, Rect};

use super::Glyph;

/// The centred icon box and stroke width a navigation glyph is drawn with.
pub(super) struct Frame {
    /// The box centre, x.
    pub(super) cx: i32,
    /// The box centre, y.
    pub(super) cy: i32,
    /// Half the box side.
    pub(super) half: i32,
    /// The box side.
    pub(super) size: i32,
    /// The stroke width.
    pub(super) stroke: f32,
}

/// Draws a navigation glyph inside `frame`, in `color`.
pub(super) fn draw(canvas: &mut dyn Canvas, glyph: Glyph, frame: Frame, color: Color) {
    let Frame {
        cx: center_x,
        cy: center_y,
        half,
        size,
        stroke,
    } = frame;
    match glyph {
        Glyph::Star => {
            let points = star_points(center_x, center_y, half);
            for index in 0..points.len() {
                canvas.draw_line(
                    points[index],
                    points[(index + 1) % points.len()],
                    color,
                    stroke,
                );
            }
        }
        Glyph::StarFilled => {
            canvas.fill_polygon(&star_points(center_x, center_y, half), color);
        }
        Glyph::Audio => {
            let (left, top, bottom) = (center_x - half, center_y - size / 4, center_y + size / 4);
            let cone = center_x - size / 4;
            canvas.fill_rect(Rect::new(left, top, cone, bottom), color);
            canvas.fill_polygon(
                &[
                    Point::new(cone, top),
                    Point::new(center_x, center_y - size / 3),
                    Point::new(center_x, center_y + size / 3),
                    Point::new(cone, bottom),
                ],
                color,
            );
        }
        Glyph::Album => {
            let radius = half as f32;
            canvas.stroke_ellipse(
                Point::new(center_x, center_y),
                radius,
                radius,
                color,
                stroke,
            );
            let spindle = (size / 8).max(2) as f32;
            canvas.fill_ellipse(Point::new(center_x, center_y), spindle, spindle, color);
        }
        Glyph::People => {
            let head = (size / 6).max(2) as f32;
            let shoulder = (size / 8).max(2) as f32;
            canvas.fill_ellipse(
                Point::new(center_x - size / 6, center_y - size / 7),
                head,
                head,
                color,
            );
            canvas.fill_ellipse(
                Point::new(center_x + size / 6, center_y - size / 9),
                head,
                head,
                color,
            );
            canvas.fill_rounded_rect(
                Rect::new(
                    center_x - size / 2,
                    center_y + size / 16,
                    center_x,
                    center_y + half,
                ),
                shoulder,
                color,
            );
            canvas.fill_rounded_rect(
                Rect::new(
                    center_x,
                    center_y + size / 8,
                    center_x + half,
                    center_y + half,
                ),
                shoulder,
                color,
            );
        }
        Glyph::Tag => {
            let (left, right) = (center_x - half, center_x + half);
            let (top, bottom) = (center_y - half, center_y + half);
            canvas.fill_polygon(
                &[
                    Point::new(left, center_y),
                    Point::new(left + size / 3, top),
                    Point::new(right - size / 4, top),
                    Point::new(right, top + size / 4),
                    Point::new(right, bottom - size / 4),
                    Point::new(right - size / 4, bottom),
                    Point::new(left + size / 3, bottom),
                ],
                color,
            );
        }
        Glyph::Folder => {
            let (left, right) = (center_x - half, center_x + half);
            let (top, bottom) = (center_y - size / 5, center_y + size / 3);
            canvas.fill_rect(Rect::new(left, top, right, bottom), color);
            canvas.fill_rect(Rect::new(left, top - size / 6, left + size / 2, top), color);
        }
        Glyph::History => {
            let radius = half as f32 * 0.8;
            canvas.stroke_ellipse(
                Point::new(center_x, center_y),
                radius,
                radius,
                color,
                stroke,
            );
            canvas.draw_line(
                Point::new(center_x, center_y),
                Point::new(center_x, center_y - size / 3),
                color,
                stroke,
            );
            canvas.draw_line(
                Point::new(center_x, center_y),
                Point::new(center_x + size / 4, center_y),
                color,
                stroke,
            );
        }
        Glyph::Monitor => {
            let (left, right) = (center_x - half, center_x + half);
            let (top, bottom) = (center_y - half, center_y + size / 8);
            canvas.stroke_rect(Rect::new(left, top, right, bottom), color, stroke);
            let stem = (size / 8).max(1);
            canvas.fill_rect(
                Rect::new(
                    center_x - stem,
                    bottom,
                    center_x + stem,
                    center_y + half - 2,
                ),
                color,
            );
            canvas.fill_rect(
                Rect::new(
                    center_x - size / 3,
                    center_y + half - 2,
                    center_x + size / 3,
                    center_y + half,
                ),
                color,
            );
        }
        Glyph::Settings => {
            let radius = (half as f32 * 0.5).max(2.0);
            canvas.stroke_ellipse(
                Point::new(center_x, center_y),
                radius,
                radius,
                color,
                stroke,
            );
            let inner = (radius - stroke).max(1.0);
            let outer = radius + (size / 8) as f32;
            for tooth in 0..8 {
                let angle = tooth as f32 * std::f32::consts::FRAC_PI_4;
                let (sin, cos) = angle.sin_cos();
                canvas.draw_line(
                    Point::new(
                        center_x + (inner * cos).round() as i32,
                        center_y + (inner * sin).round() as i32,
                    ),
                    Point::new(
                        center_x + (outer * cos).round() as i32,
                        center_y + (outer * sin).round() as i32,
                    ),
                    color,
                    stroke,
                );
            }
        }
        _ => {}
    }
}

/// The ten vertices of a five-pointed star centred at `(center_x, center_y)`
/// with outer radius `half`, alternating outer and inner points.
fn star_points(center_x: i32, center_y: i32, half: i32) -> [Point; 10] {
    let mut points = [Point::default(); 10];
    let inner = half as f32 * 0.4;
    for (index, point) in points.iter_mut().enumerate() {
        let angle = -std::f32::consts::FRAC_PI_2 + index as f32 * std::f32::consts::PI / 5.0;
        let radius = if index % 2 == 0 { half as f32 } else { inner };
        *point = Point::new(
            center_x + (radius * angle.cos()).round() as i32,
            center_y + (radius * angle.sin()).round() as i32,
        );
    }
    points
}
