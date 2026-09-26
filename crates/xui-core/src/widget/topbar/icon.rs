#![forbid(unsafe_code)]

//! The built-in [`Glyph`](super::Glyph) icon set, drawn with portable
//! [`Canvas`] primitives.

use crate::backend::{Canvas, TextStyle};
use crate::color::Color;
use crate::geometry::{Point, Rect};
use crate::units::Dip;

use super::Glyph;

/// The design size of a drawn glyph box.
const GLYPH: Dip = Dip(16.0);
/// The design size of a text glyph.
const TEXT_SIZE: Dip = Dip(14.0);

/// Draws `glyph` centred in `rect` in `color`.
///
/// A [`Glyph::Text`] run goes through [`Canvas::draw_text`]; every other glyph
/// is composed from lines, ellipses and a polygon, so the icon set needs no
/// image dependency.
pub(super) fn draw_glyph(
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
        Glyph::Star => {
            let mut points = [Point::default(); 10];
            let inner = half as f32 * 0.4;
            for (index, point) in points.iter_mut().enumerate() {
                let angle =
                    -std::f32::consts::FRAC_PI_2 + index as f32 * std::f32::consts::PI / 5.0;
                let radius = if index % 2 == 0 { half as f32 } else { inner };
                *point = Point::new(
                    center_x + (radius * angle.cos()).round() as i32,
                    center_y + (radius * angle.sin()).round() as i32,
                );
            }
            canvas.fill_polygon(&points, color);
        }
        Glyph::Text(_) => {}
    }
}
