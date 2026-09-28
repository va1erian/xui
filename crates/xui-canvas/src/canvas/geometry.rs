#![forbid(unsafe_code)]

//! [`SkiaCanvas`]'s transform, clip-mask and primitive-drawing helpers, split
//! from `canvas.rs` so both files stay under the size limit.

use tiny_skia::{Mask, Path, PathBuilder, Shader, Transform};

use xui_core::backend::{Corner, Stroke};
use xui_core::geometry::{Point, Rect};

use super::SkiaCanvas;
use crate::paint::{Clip, FILL_RULE, intersect, paint, sk_rect, skia_stroke, solid_shader};

impl SkiaCanvas<'_> {
    pub(super) fn point(&self, point: Point) -> Point {
        Point::new(
            (self.tx + point.x as f32 * self.scale).round() as i32,
            (self.ty + point.y as f32 * self.scale).round() as i32,
        )
    }

    /// Maps `rect` into device space, ignoring the clip. Text alignment is
    /// relative to the whole target rect, so it must not be trimmed first.
    pub(super) fn map_rect(&self, rect: Rect) -> Rect {
        Rect::new(
            (self.tx + rect.left as f32 * self.scale).round() as i32,
            (self.ty + rect.top as f32 * self.scale).round() as i32,
            (self.tx + rect.right as f32 * self.scale).round() as i32,
            (self.ty + rect.bottom as f32 * self.scale).round() as i32,
        )
    }

    /// Maps `rect` into device space and trims it to the innermost clip.
    pub(super) fn rect(&self, rect: Rect) -> Rect {
        let mapped = self.map_rect(rect);
        self.clips
            .last()
            .map_or(mapped, |clip| intersect(mapped, clip.bounds()))
    }

    pub(super) fn corners(&self, corners: [Corner; 4]) -> [Corner; 4] {
        corners.map(|corner| Corner::new(corner.x * self.scale, corner.y * self.scale))
    }

    /// Builds a coverage mask for the current clip stack, or `None` when there
    /// are no clips. Start fully opaque, then intersect every clip outline.
    fn build_mask(&self) -> Option<Mask> {
        let mut mask = Mask::new(self.pixmap.width(), self.pixmap.height())?;
        mask.clear();
        mask.invert();
        for clip in &self.clips {
            if let Some(path) = clip.path() {
                mask.intersect_path(&path, FILL_RULE, true, Transform::identity());
            }
        }
        Some(mask)
    }

    /// Rebuilds the clip mask after a push/pop. A rounded clip always needs
    /// one; a purely rectangular clip is trimmed by intersecting each shape's
    /// rectangle, which is exact for rectangles and cheaper, so it waits for
    /// [`SkiaCanvas::ensure_mask`] rather than allocating a mask per push.
    pub(super) fn rebuild_mask(&mut self) {
        let rounded = self
            .clips
            .iter()
            .any(|clip| matches!(clip, Clip::Rounded(..)));
        self.mask = if rounded { self.build_mask() } else { None };
    }

    /// Ensures a mask exists for a shape the cheap rectangle intersection
    /// cannot trim (an ellipse or polygon). A rounded clip already has one.
    pub(super) fn ensure_mask(&mut self) {
        if self.mask.is_none() && !self.clips.is_empty() {
            self.mask = self.build_mask();
        }
    }

    /// Fills `path`, whose coordinates are already in device space (the
    /// transform is applied by [`SkiaCanvas::rect`]/[`SkiaCanvas::point`]).
    pub(super) fn fill(&mut self, path: &Path, shader: Shader<'static>) {
        let paint = paint(shader);
        let mask = self.mask.as_ref();
        self.pixmap
            .fill_path(path, &paint, FILL_RULE, Transform::identity(), mask);
    }

    pub(super) fn stroke(&mut self, path: &Path, color: tiny_skia::Color, stroke: &Stroke) {
        let paint = paint(solid_shader(color));
        let stroke = skia_stroke(stroke, self.scale);
        let mask = self.mask.as_ref();
        self.pixmap
            .stroke_path(path, &paint, &stroke, Transform::identity(), mask);
    }

    /// The ellipse path at `center` with radii `radius_x`/`radius_y`, culled
    /// against the innermost clip.
    pub(super) fn ellipse_path(&self, center: Point, radius_x: f32, radius_y: f32) -> Option<Path> {
        let center = self.point(center);
        let bounds = Rect::new(
            (center.x as f32 - radius_x * self.scale).round() as i32,
            (center.y as f32 - radius_y * self.scale).round() as i32,
            (center.x as f32 + radius_x * self.scale).round() as i32,
            (center.y as f32 + radius_y * self.scale).round() as i32,
        );
        if bounds.is_empty() {
            return None;
        }
        let mut builder = PathBuilder::new();
        builder.push_oval(sk_rect(bounds));
        builder.finish()
    }
}
