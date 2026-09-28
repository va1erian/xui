#![forbid(unsafe_code)]

//! Vendored [Lucide](https://lucide.dev) icons (ISC, see
//! `assets/lucide/README.md`), drawn as native vector paths with
//! [`Canvas::stroke_path`] and [`Canvas::fill_path`].

#[rustfmt::skip]
#[allow(clippy::approx_constant)] // generated coordinates, not maths
mod data;

use crate::backend::Cap;
use crate::backend::{Canvas, Join, PathPlacement, PathSeg, Rgba, Stroke};
use crate::color::Color;
use crate::geometry::Rect;

pub(crate) use data::*;

/// The Lucide design grid the outlines are authored on.
const GRID: f32 = 24.0;
/// The stroke width on that grid (Lucide's own is 2; a hair thinner keeps the
/// dense icons open at small sizes).
const STROKE: f32 = 1.75;

/// Draws `path` centred in `rect`, scaled so the 24-unit grid spans `size`
/// pixels. `filled` also fills the outline (transport shapes, filled stars),
/// which is how Lucide's own "fill" variants are meant to be used.
pub(crate) fn draw(
    canvas: &mut dyn Canvas,
    path: &[PathSeg],
    filled: bool,
    rect: Rect,
    size: i32,
    color: Color,
    dpi: u32,
) {
    let scale = size as f32 / GRID;
    let width = (STROKE * scale).max(1.25 * dpi as f32 / 96.0);
    let at = PathPlacement::new(
        scale,
        (rect.left + rect.right - size) as f32 / 2.0,
        (rect.top + rect.bottom - size) as f32 / 2.0,
    );
    let color = Rgba::from(color);
    if filled {
        canvas.fill_path(path, at, color);
    }
    let stroke = Stroke::new(width).cap(Cap::Round).join(Join::Round);
    canvas.stroke_path(path, at, color, &stroke);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_outline_stays_on_the_grid() {
        for path in [SETTINGS, PLAY, SEARCH, USERS, DISC] {
            for seg in path {
                let coords: &[f32] = match seg {
                    PathSeg::MoveTo(x, y) | PathSeg::LineTo(x, y) => &[*x, *y],
                    PathSeg::CubicTo(a, b, c, d, e, f) => &[*a, *b, *c, *d, *e, *f],
                    PathSeg::Close => &[],
                };
                assert!(coords.iter().all(|v| (0.0..=GRID).contains(v)), "{seg:?}");
            }
        }
    }
}
