#![forbid(unsafe_code)]

//! The portable [`Canvas`] over a `tiny-skia` pixmap.
//!
//! Shapes are drawn with a tracked translation/scale and a clip. A rectangular
//! clip is exact for axis-aligned rectangles (each is intersected with it) and
//! trims text and images by bounds; a rounded clip, or any clip an ellipse or
//! polygon must be cut by, builds a full-surface coverage mask so the corners
//! and edges clip exactly.

use tiny_skia::{
    FilterQuality, Mask, Paint, Path, PathBuilder, Pattern, Pixmap, Shader, SpreadMode, Transform,
};

use xui_core::backend::{
    Canvas, Corner, LinearGradient, RadialGradient, Rgba, Stroke, TextLayout, TextStyle,
};
use xui_core::color::Color;
use xui_core::geometry::{Point, Rect};
use xui_core::image::Image;

use crate::image_cache::ImageCache;
use crate::paint::{
    Clip, FILL_RULE, corners_path, intersect, linear_shader, paint, radial_shader, rect_path,
    sk_rect, skia_stroke, solid_shader,
};
use crate::{to_skia, to_skia_rgba};

/// A drawing surface over a `tiny-skia` pixmap, clipped to `bounds`.
pub struct SkiaCanvas<'a> {
    pixmap: &'a mut Pixmap,
    /// The surface's decoded images, so a repeat draw is a cached blit.
    images: &'a mut ImageCache,
    bounds: Rect,
    dpi: u32,
    tx: f32,
    ty: f32,
    scale: f32,
    clips: Vec<Clip>,
    mask: Option<Mask>,
    saved: Vec<(f32, f32, f32)>,
}

impl<'a> SkiaCanvas<'a> {
    pub(crate) fn new(
        pixmap: &'a mut Pixmap,
        images: &'a mut ImageCache,
        bounds: Rect,
        dpi: u32,
    ) -> SkiaCanvas<'a> {
        SkiaCanvas {
            pixmap,
            images,
            bounds,
            dpi,
            tx: 0.0,
            ty: 0.0,
            scale: 1.0,
            clips: Vec::new(),
            mask: None,
            saved: Vec::new(),
        }
    }

    fn point(&self, point: Point) -> Point {
        Point::new(
            (self.tx + point.x as f32 * self.scale).round() as i32,
            (self.ty + point.y as f32 * self.scale).round() as i32,
        )
    }

    /// Maps `rect` into device space, ignoring the clip. Text alignment is
    /// relative to the whole target rect, so it must not be trimmed first.
    fn map_rect(&self, rect: Rect) -> Rect {
        Rect::new(
            (self.tx + rect.left as f32 * self.scale).round() as i32,
            (self.ty + rect.top as f32 * self.scale).round() as i32,
            (self.tx + rect.right as f32 * self.scale).round() as i32,
            (self.ty + rect.bottom as f32 * self.scale).round() as i32,
        )
    }

    /// Maps `rect` into device space and trims it to the innermost clip.
    fn rect(&self, rect: Rect) -> Rect {
        let mapped = self.map_rect(rect);
        self.clips
            .last()
            .map_or(mapped, |clip| intersect(mapped, clip.bounds()))
    }

    fn corners(&self, corners: [Corner; 4]) -> [Corner; 4] {
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
    fn rebuild_mask(&mut self) {
        let rounded = self
            .clips
            .iter()
            .any(|clip| matches!(clip, Clip::Rounded(..)));
        self.mask = if rounded { self.build_mask() } else { None };
    }

    /// Ensures a mask exists for a shape the cheap rectangle intersection
    /// cannot trim (an ellipse or polygon). A rounded clip already has one.
    fn ensure_mask(&mut self) {
        if self.mask.is_none() && !self.clips.is_empty() {
            self.mask = self.build_mask();
        }
    }

    /// Fills `path`, whose coordinates are already in device space (the
    /// transform is applied by [`SkiaCanvas::rect`]/[`SkiaCanvas::point`]).
    fn fill(&mut self, path: &Path, shader: Shader<'static>) {
        let paint = paint(shader);
        let mask = self.mask.as_ref();
        self.pixmap
            .fill_path(path, &paint, FILL_RULE, Transform::identity(), mask);
    }

    fn stroke(&mut self, path: &Path, color: tiny_skia::Color, stroke: &Stroke) {
        let paint = paint(solid_shader(color));
        let stroke = skia_stroke(stroke, self.scale);
        let mask = self.mask.as_ref();
        self.pixmap
            .stroke_path(path, &paint, &stroke, Transform::identity(), mask);
    }
}

impl Canvas for SkiaCanvas<'_> {
    fn dpi(&self) -> u32 {
        self.dpi
    }

    fn bounds(&self) -> Rect {
        self.bounds
    }

    fn clear(&mut self, color: Color) {
        self.fill_rect(self.bounds, color);
    }

    fn fill_rect(&mut self, rect: Rect, color: Color) {
        if let Some(path) = rect_path(self.rect(rect)) {
            self.fill(&path, solid_shader(to_skia(color)));
        }
    }

    fn fill_rounded_rect(&mut self, rect: Rect, radius: f32, color: Color) {
        let rect = self.rect(rect);
        if let Some(path) = corners_path(rect, self.corners([Corner::uniform(radius); 4])) {
            self.fill(&path, solid_shader(to_skia(color)));
        }
    }

    fn fill_ellipse(&mut self, center: Point, radius_x: f32, radius_y: f32, color: Color) {
        // The full ellipse, not one trimmed to the clip's bounding box: the
        // mask trims it exactly, so a clipped edge stays straight.
        let Some(path) = self.ellipse_path(center, radius_x, radius_y) else {
            return;
        };
        self.ensure_mask();
        self.fill(&path, solid_shader(to_skia(color)));
    }

    fn fill_polygon(&mut self, points: &[Point], color: Color) {
        if points.len() < 3 {
            return;
        }
        let first = self.point(points[0]);
        let mut builder = PathBuilder::new();
        builder.move_to(first.x as f32, first.y as f32);
        for point in &points[1..] {
            let point = self.point(*point);
            builder.line_to(point.x as f32, point.y as f32);
        }
        builder.close();
        if let Some(path) = builder.finish() {
            self.ensure_mask();
            self.fill(&path, solid_shader(to_skia(color)));
        }
    }

    fn stroke_rect(&mut self, rect: Rect, color: Color, width: f32) {
        if let Some(path) = rect_path(self.rect(rect)) {
            self.stroke(&path, to_skia(color), &Stroke::new(width));
        }
    }

    fn stroke_rounded_rect(&mut self, rect: Rect, radius: f32, color: Color, width: f32) {
        let rect = self.rect(rect);
        if let Some(path) = corners_path(rect, self.corners([Corner::uniform(radius); 4])) {
            self.stroke(&path, to_skia(color), &Stroke::new(width));
        }
    }

    fn stroke_ellipse(
        &mut self,
        center: Point,
        radius_x: f32,
        radius_y: f32,
        color: Color,
        width: f32,
    ) {
        if let Some(path) = self.ellipse_path(center, radius_x, radius_y) {
            self.stroke(&path, to_skia(color), &Stroke::new(width));
        }
    }

    fn draw_line(&mut self, from: Point, to: Point, color: Color, width: f32) {
        self.draw_line_stroked(from, to, color.into(), &Stroke::new(width));
    }

    fn fill_rect_rgba(&mut self, rect: Rect, color: Rgba) {
        if let Some(path) = rect_path(self.rect(rect)) {
            self.fill(&path, solid_shader(to_skia_rgba(color)));
        }
    }

    fn fill_rounded_rect_corners(&mut self, rect: Rect, corners: [Corner; 4], color: Rgba) {
        let rect = self.rect(rect);
        if let Some(path) = corners_path(rect, self.corners(corners)) {
            self.fill(&path, solid_shader(to_skia_rgba(color)));
        }
    }

    fn stroke_rounded_rect_corners(
        &mut self,
        rect: Rect,
        corners: [Corner; 4],
        color: Rgba,
        stroke: &Stroke,
    ) {
        let rect = self.rect(rect);
        if let Some(path) = corners_path(rect, self.corners(corners)) {
            self.stroke(&path, to_skia_rgba(color), stroke);
        }
    }

    fn draw_line_stroked(&mut self, from: Point, to: Point, color: Rgba, stroke: &Stroke) {
        let (from, to) = (self.point(from), self.point(to));
        let mut builder = PathBuilder::new();
        builder.move_to(from.x as f32, from.y as f32);
        builder.line_to(to.x as f32, to.y as f32);
        if let Some(path) = builder.finish() {
            self.stroke(&path, to_skia_rgba(color), stroke);
        }
    }

    fn stroke_ellipse_stroked(
        &mut self,
        center: Point,
        radius_x: f32,
        radius_y: f32,
        color: Rgba,
        stroke: &Stroke,
    ) {
        if let Some(path) = self.ellipse_path(center, radius_x, radius_y) {
            self.stroke(&path, to_skia_rgba(color), stroke);
        }
    }

    fn fill_rect_linear(&mut self, rect: Rect, gradient: &LinearGradient) {
        if let Some(path) = rect_path(self.rect(rect))
            && let Some(shader) = linear_shader(
                self.point(gradient.start),
                self.point(gradient.end),
                &gradient.stops,
            )
        {
            self.fill(&path, shader);
        }
    }

    fn fill_rect_radial(&mut self, rect: Rect, gradient: &RadialGradient) {
        if let Some(path) = rect_path(self.rect(rect))
            && let Some(shader) = radial_shader(
                self.point(gradient.center),
                gradient.radius_x * self.scale,
                gradient.radius_y * self.scale,
                &gradient.stops,
            )
        {
            self.fill(&path, shader);
        }
    }

    fn draw_text(&mut self, text: &str, rect: Rect, style: &TextStyle) {
        let rect = self.map_rect(rect);
        let clip = self.clips.last().map(Clip::bounds);
        let dpi = self.dpi;
        crate::text::draw(
            self.pixmap,
            text,
            rect,
            style,
            dpi,
            crate::text::GlyphClip {
                bounds: clip,
                mask: self.mask.as_ref(),
            },
        );
    }

    fn draw_layout(&mut self, layout: &dyn TextLayout, origin: Point, color: Rgba) {
        let Some(cosmic) = layout
            .as_any()
            .downcast_ref::<crate::text_layout::CosmicLayout>()
        else {
            return;
        };
        let origin = self.point(origin);
        let clip = self.clips.last().map(Clip::bounds);
        crate::text_layout::draw_layout(
            self.pixmap,
            cosmic,
            origin,
            color,
            crate::text::GlyphClip {
                bounds: clip,
                mask: self.mask.as_ref(),
            },
        );
    }

    fn draw_image(&mut self, image: &Image, rect: Rect) {
        let dest = self.map_rect(rect);
        if dest.is_empty() {
            return;
        }
        // Cropped, not squashed: only the source sub-rectangle under the clip
        // is drawn, at the full-size scale, so the image's left region stays at
        // the left edge when the right half is clipped away.
        let visible = self
            .clips
            .last()
            .map_or(dest, |clip| intersect(dest, clip.bounds()));
        if visible.is_empty() {
            return;
        }
        // The premultiplied upload is cached on the surface (LRU over bytes,
        // keyed by the image's identity, which clones keep), so the same row
        // icon costs one pattern blit per repaint instead of a fresh pixmap.
        let Some(pixmap) = self.images.pixmap(image) else {
            return;
        };
        let pixmap: &Pixmap = &pixmap;
        // Map the image's own pixel rectangle onto the full destination
        // rectangle; the filled source is only the visible slice of that map.
        let scale_x = dest.width() as f32 / image.width() as f32;
        let scale_y = dest.height() as f32 / image.height() as f32;
        let transform = Transform::from_row(
            scale_x,
            0.0,
            0.0,
            scale_y,
            dest.left as f32,
            dest.top as f32,
        );
        let source = sk_rect(Rect::new(
            ((visible.left - dest.left) as f32 / scale_x).round() as i32,
            ((visible.top - dest.top) as f32 / scale_y).round() as i32,
            ((visible.right - dest.left) as f32 / scale_x).round() as i32,
            ((visible.bottom - dest.top) as f32 / scale_y).round() as i32,
        ));
        let paint = Paint {
            shader: Pattern::new(
                pixmap.as_ref(),
                SpreadMode::Pad,
                FilterQuality::Bilinear,
                1.0,
                Transform::identity(),
            ),
            anti_alias: true,
            ..Paint::default()
        };
        self.pixmap
            .fill_rect(source, &paint, transform, self.mask.as_ref());
    }

    fn push_clip(&mut self, rect: Rect) {
        let rect = self.rect(rect);
        let clipped = self
            .clips
            .last()
            .map_or(rect, |outer| intersect(outer.bounds(), rect));
        self.clips.push(Clip::Rect(clipped));
        self.rebuild_mask();
    }

    fn push_clip_rounded(&mut self, rect: Rect, corners: [Corner; 4]) {
        let rect = self.rect(rect);
        let clipped = self
            .clips
            .last()
            .map_or(rect, |outer| intersect(outer.bounds(), rect));
        self.clips
            .push(Clip::Rounded(clipped, self.corners(corners)));
        self.rebuild_mask();
    }

    fn pop_clip(&mut self) {
        self.clips.pop();
        self.rebuild_mask();
    }

    fn save(&mut self) {
        self.saved.push((self.tx, self.ty, self.scale));
    }

    fn restore(&mut self) {
        if let Some((tx, ty, scale)) = self.saved.pop() {
            self.tx = tx;
            self.ty = ty;
            self.scale = scale;
        }
    }

    fn set_translation(&mut self, x: f32, y: f32) {
        self.tx = x;
        self.ty = y;
    }

    fn set_scale_translate(&mut self, scale: f32, x: f32, y: f32) {
        self.scale = scale;
        self.tx = x;
        self.ty = y;
    }
}

impl SkiaCanvas<'_> {
    /// The ellipse path at `center` with radii `radius_x`/`radius_y`, culled
    /// against the innermost clip.
    fn ellipse_path(&self, center: Point, radius_x: f32, radius_y: f32) -> Option<Path> {
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
