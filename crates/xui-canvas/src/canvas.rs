#![forbid(unsafe_code)]

//! The portable [`Canvas`] over a `tiny-skia` pixmap.
//!
//! Shapes are drawn with a tracked translation/scale and a soft clip (each
//! shape's rectangle is intersected with the current clip), matching the Win32
//! backend. Text rasterisation is not implemented yet, so [`Canvas::draw_text`]
//! draws nothing until the text system lands.

use tiny_skia::{
    FillRule, FilterQuality, Paint, Path, PathBuilder, Pattern, Pixmap, Shader, SpreadMode, Stroke,
    Transform,
};

use xui_core::backend::{Canvas, TextStyle};
use xui_core::color::Color;
use xui_core::geometry::{Point, Rect};
use xui_core::image::Image;

use crate::to_skia;

/// A drawing surface over a `tiny-skia` pixmap, clipped to `bounds`.
pub struct SkiaCanvas<'a> {
    pixmap: &'a mut Pixmap,
    bounds: Rect,
    dpi: u32,
    tx: f32,
    ty: f32,
    scale: f32,
    clips: Vec<Rect>,
    saved: Vec<(f32, f32, f32)>,
}

impl<'a> SkiaCanvas<'a> {
    pub(crate) fn new(pixmap: &'a mut Pixmap, bounds: Rect, dpi: u32) -> SkiaCanvas<'a> {
        SkiaCanvas {
            pixmap,
            bounds,
            dpi,
            tx: 0.0,
            ty: 0.0,
            scale: 1.0,
            clips: Vec::new(),
            saved: Vec::new(),
        }
    }

    fn point(&self, point: Point) -> Point {
        Point::new(
            (self.tx + point.x as f32 * self.scale).round() as i32,
            (self.ty + point.y as f32 * self.scale).round() as i32,
        )
    }

    fn rect(&self, rect: Rect) -> Rect {
        let mapped = Rect::new(
            (self.tx + rect.left as f32 * self.scale).round() as i32,
            (self.ty + rect.top as f32 * self.scale).round() as i32,
            (self.tx + rect.right as f32 * self.scale).round() as i32,
            (self.ty + rect.bottom as f32 * self.scale).round() as i32,
        );
        self.clips
            .last()
            .copied()
            .map_or(mapped, |clip| intersect(mapped, clip))
    }

    /// Fills `path`, whose coordinates are already in device space (the
    /// transform is applied by [`SkiaCanvas::rect`]/[`SkiaCanvas::point`]).
    fn fill(&mut self, path: &Path, color: Color) {
        let paint = Paint {
            shader: Shader::SolidColor(to_skia(color)),
            anti_alias: true,
            ..Paint::default()
        };
        self.pixmap
            .fill_path(path, &paint, FillRule::Winding, Transform::identity(), None);
    }

    fn stroke(&mut self, path: &Path, color: Color, width: f32) {
        let paint = Paint {
            shader: Shader::SolidColor(to_skia(color)),
            anti_alias: true,
            ..Paint::default()
        };
        let stroke = Stroke {
            width: (width * self.scale).max(1.0),
            ..Stroke::default()
        };
        self.pixmap
            .stroke_path(path, &paint, &stroke, Transform::identity(), None);
    }
}

fn rect_path(rect: Rect) -> Option<Path> {
    if rect.is_empty() {
        return None;
    }
    Some(PathBuilder::from_rect(sk_rect(rect)))
}

/// A rounded-rectangle path: four straight edges joined by quadratic corners.
fn round_rect_path(rect: Rect, radius: f32) -> Option<Path> {
    if rect.is_empty() {
        return None;
    }
    let (l, t) = (rect.left as f32, rect.top as f32);
    let (r, b) = (rect.right as f32, rect.bottom as f32);
    let radius = radius.max(0.0).min((r - l) / 2.0).min((b - t) / 2.0);
    let mut builder = PathBuilder::new();
    builder.move_to(l + radius, t);
    builder.line_to(r - radius, t);
    builder.quad_to(r, t, r, t + radius);
    builder.line_to(r, b - radius);
    builder.quad_to(r, b, r - radius, b);
    builder.line_to(l + radius, b);
    builder.quad_to(l, b, l, b - radius);
    builder.line_to(l, t + radius);
    builder.quad_to(l, t, l + radius, t);
    builder.close();
    builder.finish()
}

fn sk_rect(rect: Rect) -> tiny_skia::Rect {
    tiny_skia::Rect::from_ltrb(
        rect.left as f32,
        rect.top as f32,
        rect.right as f32,
        rect.bottom as f32,
    )
    .unwrap_or_else(|| tiny_skia::Rect::from_ltrb(0.0, 0.0, 0.0, 0.0).expect("empty rect"))
}

fn intersect(a: Rect, b: Rect) -> Rect {
    Rect::new(
        a.left.max(b.left),
        a.top.max(b.top),
        a.right.min(b.right),
        a.bottom.min(b.bottom),
    )
}

/// Uploads `image` to a tiny-skia pixmap, premultiplying its straight alpha.
///
/// This allocates a pixmap per call; an image cache would remove that, but an
/// image is not drawn once per list row.
fn premultiplied(image: &Image) -> Option<Pixmap> {
    let mut pixmap = Pixmap::new(image.width(), image.height())?;
    let source = image.pixels().as_chunks::<4>().0;
    for (destination, pixel) in pixmap
        .data_mut()
        .as_chunks_mut::<4>()
        .0
        .iter_mut()
        .zip(source)
    {
        let alpha = pixel[3] as u32;
        let scale = |channel: u8| ((channel as u32 * alpha + 127) / 255) as u8;
        destination[0] = scale(pixel[0]);
        destination[1] = scale(pixel[1]);
        destination[2] = scale(pixel[2]);
        destination[3] = pixel[3];
    }
    Some(pixmap)
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
            self.fill(&path, color);
        }
    }

    fn fill_rounded_rect(&mut self, rect: Rect, radius: f32, color: Color) {
        let rect = self.rect(rect);
        if let Some(path) = round_rect_path(rect, radius * self.scale) {
            self.fill(&path, color);
        }
    }

    fn fill_ellipse(&mut self, center: Point, radius_x: f32, radius_y: f32, color: Color) {
        let center = self.point(center);
        let rx = radius_x * self.scale;
        let ry = radius_y * self.scale;
        let bounds = Rect::new(
            (center.x as f32 - rx).round() as i32,
            (center.y as f32 - ry).round() as i32,
            (center.x as f32 + rx).round() as i32,
            (center.y as f32 + ry).round() as i32,
        );
        let bounds = self
            .clips
            .last()
            .copied()
            .map_or(bounds, |clip| intersect(bounds, clip));
        if bounds.is_empty() {
            return;
        }
        let mut builder = PathBuilder::new();
        builder.push_oval(sk_rect(bounds));
        if let Some(path) = builder.finish() {
            self.fill(&path, color);
        }
    }

    fn fill_polygon(&mut self, points: &[Point], color: Color) {
        if points.len() < 3 {
            return;
        }
        // Like the other shapes, clip is approximated by the bounding box.
        let points: Vec<Point> = points.iter().map(|point| self.point(*point)).collect();
        if let Some(clip) = self.clips.last().copied() {
            let bbox = points.iter().fold(
                Rect::new(i32::MAX, i32::MAX, i32::MIN, i32::MIN),
                |bounds, point| {
                    Rect::new(
                        bounds.left.min(point.x),
                        bounds.top.min(point.y),
                        bounds.right.max(point.x),
                        bounds.bottom.max(point.y),
                    )
                },
            );
            if intersect(bbox, clip).is_empty() {
                return;
            }
        }
        let mut builder = PathBuilder::new();
        builder.move_to(points[0].x as f32, points[0].y as f32);
        for point in &points[1..] {
            builder.line_to(point.x as f32, point.y as f32);
        }
        builder.close();
        if let Some(path) = builder.finish() {
            self.fill(&path, color);
        }
    }

    fn stroke_rect(&mut self, rect: Rect, color: Color, width: f32) {
        if let Some(path) = rect_path(self.rect(rect)) {
            self.stroke(&path, color, width);
        }
    }

    fn stroke_rounded_rect(&mut self, rect: Rect, radius: f32, color: Color, width: f32) {
        let rect = self.rect(rect);
        if let Some(path) = round_rect_path(rect, radius * self.scale) {
            self.stroke(&path, color, width);
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
        let center = self.point(center);
        let bounds = Rect::new(
            (center.x as f32 - radius_x * self.scale).round() as i32,
            (center.y as f32 - radius_y * self.scale).round() as i32,
            (center.x as f32 + radius_x * self.scale).round() as i32,
            (center.y as f32 + radius_y * self.scale).round() as i32,
        );
        if bounds.is_empty() {
            return;
        }
        let mut builder = PathBuilder::new();
        builder.push_oval(sk_rect(bounds));
        if let Some(path) = builder.finish() {
            self.stroke(&path, color, width);
        }
    }

    fn draw_line(&mut self, from: Point, to: Point, color: Color, width: f32) {
        let (from, to) = (self.point(from), self.point(to));
        let mut builder = PathBuilder::new();
        builder.move_to(from.x as f32, from.y as f32);
        builder.line_to(to.x as f32, to.y as f32);
        if let Some(path) = builder.finish() {
            self.stroke(&path, color, width);
        }
    }

    fn draw_text(&mut self, text: &str, rect: Rect, style: &TextStyle) {
        let rect = self.rect(rect);
        let dpi = self.dpi;
        crate::text::draw(self.pixmap, text, rect, style, dpi);
    }

    fn draw_image(&mut self, image: &Image, rect: Rect) {
        let rect = self.rect(rect);
        if rect.is_empty() {
            return;
        }
        let Some(pixmap) = premultiplied(image) else {
            return;
        };
        // Map the image's own pixel rectangle onto the destination rectangle.
        let scale_x = rect.width() as f32 / image.width() as f32;
        let scale_y = rect.height() as f32 / image.height() as f32;
        let transform = Transform::from_row(
            scale_x,
            0.0,
            0.0,
            scale_y,
            rect.left as f32,
            rect.top as f32,
        );
        let source = sk_rect(Rect::new(0, 0, image.width() as i32, image.height() as i32));
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
        self.pixmap.fill_rect(source, &paint, transform, None);
    }

    fn push_clip(&mut self, rect: Rect) {
        let rect = self.rect(rect);
        let clipped = self
            .clips
            .last()
            .map_or(rect, |outer| intersect(*outer, rect));
        self.clips.push(clipped);
    }

    fn pop_clip(&mut self) {
        self.clips.pop();
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
