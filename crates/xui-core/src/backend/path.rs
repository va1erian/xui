#![forbid(unsafe_code)]

//! Vector paths for [`Canvas::fill_path`](super::Canvas::fill_path) and
//! [`Canvas::stroke_path`](super::Canvas::stroke_path): a flat list of
//! segments (so a static icon table needs no allocation), a placement that
//! scales and offsets it, and a flattener for backends that only draw lines.

use crate::geometry::Point;

/// One segment of a path, in the path's own coordinates.
///
/// A path is a list of figures: each starts at a [`MoveTo`](PathSeg::MoveTo),
/// runs through lines and curves, and may end in a [`Close`](PathSeg::Close).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PathSeg {
    /// Starts a new figure at `(x, y)`.
    MoveTo(f32, f32),
    /// A straight line to `(x, y)`.
    LineTo(f32, f32),
    /// A cubic Bézier to `(x, y)` with control points `(x1, y1)` and `(x2, y2)`,
    /// in that order: `CubicTo(x1, y1, x2, y2, x, y)`.
    CubicTo(f32, f32, f32, f32, f32, f32),
    /// Closes the current figure with a straight line back to its start.
    Close,
}

/// Where a path lands on the canvas: `canvas = origin + path * scale`, in the
/// canvas's own coordinates (the current transform still applies on top).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PathPlacement {
    /// The factor applied to the path's coordinates.
    pub scale: f32,
    /// The horizontal offset of the path's origin.
    pub x: f32,
    /// The vertical offset of the path's origin.
    pub y: f32,
}

impl PathPlacement {
    /// A placement scaling by `scale` with the path origin at `(x, y)`.
    pub const fn new(scale: f32, x: f32, y: f32) -> PathPlacement {
        PathPlacement { scale, x, y }
    }

    /// Maps a path-space point to canvas space.
    pub fn apply(self, x: f32, y: f32) -> (f32, f32) {
        (self.x + x * self.scale, self.y + y * self.scale)
    }
}

/// A flattened figure.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Polyline {
    /// The vertices, rounded to whole canvas pixels.
    pub points: Vec<Point>,
    /// Whether the figure was closed.
    pub closed: bool,
}

/// Curve subdivision used by [`flatten`].
const STEPS: usize = 12;

/// Flattens `path` into polylines in canvas space, for a backend (or a
/// fallback) without curve support.
pub fn flatten(path: &[PathSeg], at: PathPlacement) -> Vec<Polyline> {
    let mut figures: Vec<Polyline> = Vec::new();
    let mut pen = (0.0, 0.0);
    let push = |figures: &mut Vec<Polyline>, (x, y): (f32, f32)| {
        if let Some(figure) = figures.last_mut() {
            figure
                .points
                .push(Point::new(x.round() as i32, y.round() as i32));
        }
    };
    for seg in path {
        match *seg {
            PathSeg::MoveTo(x, y) => {
                pen = at.apply(x, y);
                figures.push(Polyline {
                    points: Vec::new(),
                    closed: false,
                });
                push(&mut figures, pen);
            }
            PathSeg::LineTo(x, y) => {
                pen = at.apply(x, y);
                push(&mut figures, pen);
            }
            PathSeg::CubicTo(x1, y1, x2, y2, x, y) => {
                let (c1, c2, end) = (at.apply(x1, y1), at.apply(x2, y2), at.apply(x, y));
                for step in 1..=STEPS {
                    let t = step as f32 / STEPS as f32;
                    let u = 1.0 - t;
                    let mix = |a: f32, b: f32, c: f32, d: f32| {
                        u * u * u * a + 3.0 * u * u * t * b + 3.0 * u * t * t * c + t * t * t * d
                    };
                    push(
                        &mut figures,
                        (mix(pen.0, c1.0, c2.0, end.0), mix(pen.1, c1.1, c2.1, end.1)),
                    );
                }
                pen = end;
            }
            PathSeg::Close => {
                if let Some(figure) = figures.last_mut() {
                    figure.closed = true;
                }
            }
        }
    }
    figures
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_placed_line_lands_on_the_offset() {
        let path = [PathSeg::MoveTo(0.0, 0.0), PathSeg::LineTo(10.0, 0.0)];
        let figures = flatten(&path, PathPlacement::new(2.0, 5.0, 7.0));
        assert_eq!(figures.len(), 1);
        assert_eq!(figures[0].points, [Point::new(5, 7), Point::new(25, 7)]);
        assert!(!figures[0].closed);
    }

    #[test]
    fn a_curve_ends_exactly_on_its_endpoint_and_a_close_is_recorded() {
        let path = [
            PathSeg::MoveTo(0.0, 0.0),
            PathSeg::CubicTo(0.0, 10.0, 10.0, 10.0, 10.0, 0.0),
            PathSeg::Close,
        ];
        let figure = &flatten(&path, PathPlacement::new(1.0, 0.0, 0.0))[0];
        assert_eq!(figure.points.len(), 1 + STEPS);
        assert_eq!(figure.points.last(), Some(&Point::new(10, 0)));
        assert!(figure.closed);
    }
}
