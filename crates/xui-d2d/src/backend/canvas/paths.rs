#![forbid(unsafe_code)]

//! Path placement for the canvas: a path's own scale and offset, then the
//! canvas's translation and scale.

use xui_core::backend::{PathPlacement, PathSeg};

use super::D2dCanvas;
use crate::sys::gfx::Geometry;

impl D2dCanvas {
    /// `at` composed with the canvas's own scale and translation.
    fn placed(&self, at: PathPlacement) -> PathPlacement {
        PathPlacement::new(
            at.scale * self.scale,
            self.tx + at.x * self.scale,
            self.ty + at.y * self.scale,
        )
    }

    /// The geometry of `path` as placed on this canvas.
    pub(super) fn geometry(&self, path: &[PathSeg], at: PathPlacement) -> Option<Geometry> {
        Geometry::path(path, self.placed(at))
    }

    /// A point in path space, as placed on this canvas.
    pub(super) fn path_point(&self, point: (f32, f32), at: PathPlacement) -> (f32, f32) {
        self.placed(at).apply(point.0, point.1)
    }
}
