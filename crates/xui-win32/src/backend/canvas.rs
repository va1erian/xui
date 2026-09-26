#![forbid(unsafe_code)]

//! The Win32 implementation of the portable [`Canvas`]: shapes go through the
//! GDI/Direct2D [`gdi::Canvas`], and the wrapper tracks the translation, scale
//! and clip the core contract requires.
//!
//! An axis-aligned clip is applied by intersecting each shape's rectangle with
//! the current clip; it is exact for the axis-aligned rectangles a widget
//! draws, and a stroke that straddles the clip edge is not trimmed. A rounded
//! clip is re-applied as a Direct2D layer around each shape drawn under it, so
//! its corner arcs clip too.

use crate::d2d::{PointF, RectF, Stroke as D2dStroke};
use crate::gdi;
use xui_core::backend::{Canvas, Corner, LinearGradient, RadialGradient, Rgba, Stroke, TextStyle};
use xui_core::image::Image;
use xui_core::{Color, Point, Rect};

use super::shape::{Clip, d2d_stroke, intersect, linear, point_f, radial, rgba, rounded};

/// A portable canvas over a Win32 GDI [`gdi::Canvas`].
pub(crate) struct Win32Canvas<'a> {
    pub(crate) canvas: &'a gdi::Canvas,
    bounds: Rect,
    dpi: u32,
    pub(crate) tx: f32,
    pub(crate) ty: f32,
    pub(crate) scale: f32,
    pub(crate) clips: Vec<Clip>,
    saved: Vec<(f32, f32, f32)>,
}

impl<'a> Win32Canvas<'a> {
    pub(crate) fn new(canvas: &'a gdi::Canvas, bounds: Rect, dpi: u32) -> Win32Canvas<'a> {
        Win32Canvas {
            canvas,
            bounds,
            dpi,
            tx: 0.0,
            ty: 0.0,
            scale: 1.0,
            clips: Vec::new(),
            saved: Vec::new(),
        }
    }
}

impl Canvas for Win32Canvas<'_> {
    fn dpi(&self) -> u32 {
        self.dpi
    }

    fn bounds(&self) -> Rect {
        self.bounds
    }

    fn clear(&mut self, color: Color) {
        self.canvas.fill_rect(self.bounds, color);
    }

    fn fill_rect(&mut self, rect: Rect, color: Color) {
        let rect = self.rect(rect);
        if !rect.is_empty() {
            self.canvas.fill_rect(rect, color);
        }
    }

    fn fill_rounded_rect(&mut self, rect: Rect, radius: f32, color: Color) {
        let rect = self.rect(rect);
        if !rect.is_empty() {
            self.canvas
                .round_rect(rect, radius.max(1.0) as i32, color, None);
        }
    }

    fn fill_ellipse(&mut self, center: Point, radius_x: f32, radius_y: f32, color: Color) {
        let bounds = self.ellipse_bounds(center, radius_x, radius_y);
        if self.ellipse_clipped_out(bounds) {
            return;
        }
        if let Some(mut d2d) = self.d2d() {
            self.push_rounded_clips(&mut d2d);
            let center = self.point(center);
            d2d.fill_ellipse(
                PointF::new(center.x as f32, center.y as f32),
                radius_x * self.scale,
                radius_y * self.scale,
                color,
            );
            let _ = d2d.end_draw();
            return;
        }
        // GDI has no anti-aliased ellipse: fill the clipped bounding rectangle.
        let rect = self
            .clips
            .last()
            .map_or(bounds, |clip| intersect(bounds, clip.bounds));
        if !rect.is_empty() {
            self.canvas.fill_rect(rect, color);
        }
    }

    fn fill_polygon(&mut self, points: &[Point], color: Color) {
        if points.len() < 3 {
            return;
        }
        let points: Vec<Point> = points.iter().map(|point| self.point(*point)).collect();
        self.canvas.polygon(&points, color);
    }

    fn stroke_rect(&mut self, rect: Rect, color: Color, width: f32) {
        let rect = self.rect(rect);
        if rect.is_empty() {
            return;
        }
        if let Some(mut d2d) = self.d2d() {
            self.push_rounded_clips(&mut d2d);
            d2d.stroke_rect(
                RectF::from_rect(rect),
                color,
                D2dStroke::solid(width.max(1.0)),
            );
            let _ = d2d.end_draw();
            return;
        }
        self.canvas.outline(rect, color);
    }

    fn stroke_rounded_rect(&mut self, rect: Rect, radius: f32, color: Color, width: f32) {
        let rect = self.rect(rect);
        if rect.is_empty() {
            return;
        }
        if let Some(mut d2d) = self.d2d() {
            self.push_rounded_clips(&mut d2d);
            d2d.stroke_rounded_rect(
                RectF::from_rect(rect),
                radius.max(1.0),
                color,
                D2dStroke::solid(width.max(1.0)),
            );
            let _ = d2d.end_draw();
            return;
        }
        self.canvas.outline(rect, color);
    }

    fn stroke_ellipse(
        &mut self,
        center: Point,
        radius_x: f32,
        radius_y: f32,
        color: Color,
        width: f32,
    ) {
        if self.ellipse_clipped_out(self.ellipse_bounds(center, radius_x, radius_y)) {
            return;
        }
        // GDI has no anti-aliased ellipse outline, so without Direct2D there is
        // nothing to draw; every backend that can is expected to provide D2D.
        if let Some(mut d2d) = self.d2d() {
            self.push_rounded_clips(&mut d2d);
            let center = self.point(center);
            d2d.stroke_ellipse(
                PointF::new(center.x as f32, center.y as f32),
                radius_x * self.scale,
                radius_y * self.scale,
                color,
                D2dStroke::solid(width.max(1.0)),
            );
            let _ = d2d.end_draw();
        }
    }

    fn draw_line(&mut self, from: Point, to: Point, color: Color, width: f32) {
        let (from, to) = (self.point(from), self.point(to));
        self.canvas.line(from, to, color, width.max(1.0) as i32);
    }

    fn fill_rect_rgba(&mut self, rect: Rect, color: Rgba) {
        let rect = self.rect(rect);
        if rect.is_empty() {
            return;
        }
        if let Some(mut d2d) = self.d2d() {
            self.push_rounded_clips(&mut d2d);
            d2d.fill_rect_rgba(RectF::from_rect(rect), rgba(color));
            let _ = d2d.end_draw();
            return;
        }
        // GDI has no alpha; fall back to the opaque colour.
        self.canvas
            .fill_rect(rect, Color::rgb(color.r, color.g, color.b));
    }

    fn fill_rounded_rect_corners(&mut self, rect: Rect, corners: [Corner; 4], color: Rgba) {
        let rect = self.rect(rect);
        if rect.is_empty() {
            return;
        }
        if let Some(mut d2d) = self.d2d() {
            self.push_rounded_clips(&mut d2d);
            d2d.fill_rounded(rounded(rect, corners), rgba(color));
            let _ = d2d.end_draw();
            return;
        }
        let radius = corners
            .iter()
            .map(|corner| corner.x.max(corner.y))
            .fold(0.0_f32, f32::max)
            .max(1.0) as i32;
        self.canvas
            .round_rect(rect, radius, Color::rgb(color.r, color.g, color.b), None);
    }

    fn stroke_rounded_rect_corners(
        &mut self,
        rect: Rect,
        corners: [Corner; 4],
        color: Rgba,
        stroke: &Stroke,
    ) {
        let rect = self.rect(rect);
        if rect.is_empty() {
            return;
        }
        if let Some(mut d2d) = self.d2d() {
            self.push_rounded_clips(&mut d2d);
            d2d.stroke_rounded(rounded(rect, corners), rgba(color), d2d_stroke(stroke));
            let _ = d2d.end_draw();
            return;
        }
        self.canvas
            .outline(rect, Color::rgb(color.r, color.g, color.b));
    }

    fn draw_line_stroked(&mut self, from: Point, to: Point, color: Rgba, stroke: &Stroke) {
        let (from, to) = (self.point(from), self.point(to));
        if let Some(mut d2d) = self.d2d() {
            self.push_rounded_clips(&mut d2d);
            d2d.draw_line_rgba(point_f(from), point_f(to), rgba(color), d2d_stroke(stroke));
            let _ = d2d.end_draw();
            return;
        }
        // GDI has no dash pattern; draw the line solid.
        self.canvas.line(
            from,
            to,
            Color::rgb(color.r, color.g, color.b),
            stroke.width.max(1.0) as i32,
        );
    }

    fn stroke_ellipse_stroked(
        &mut self,
        center: Point,
        radius_x: f32,
        radius_y: f32,
        color: Rgba,
        stroke: &Stroke,
    ) {
        if self.ellipse_clipped_out(self.ellipse_bounds(center, radius_x, radius_y)) {
            return;
        }
        if let Some(mut d2d) = self.d2d() {
            self.push_rounded_clips(&mut d2d);
            let center = self.point(center);
            d2d.stroke_ellipse_rgba(
                PointF::new(center.x as f32, center.y as f32),
                radius_x * self.scale,
                radius_y * self.scale,
                rgba(color),
                d2d_stroke(stroke),
            );
            let _ = d2d.end_draw();
        }
    }

    fn fill_rect_linear(&mut self, rect: Rect, gradient: &LinearGradient) {
        let rect = self.rect(rect);
        if rect.is_empty() {
            return;
        }
        if let Some(mut d2d) = self.d2d() {
            self.push_rounded_clips(&mut d2d);
            d2d.fill_rect_linear(RectF::from_rect(rect), &linear(gradient));
            let _ = d2d.end_draw();
            return;
        }
        self.fill_first_stop(rect, gradient.stops.first());
    }

    fn fill_rect_radial(&mut self, rect: Rect, gradient: &RadialGradient) {
        let rect = self.rect(rect);
        if rect.is_empty() {
            return;
        }
        if let Some(mut d2d) = self.d2d() {
            self.push_rounded_clips(&mut d2d);
            d2d.fill_rect_radial(RectF::from_rect(rect), &radial(gradient, self.scale));
            let _ = d2d.end_draw();
            return;
        }
        self.fill_first_stop(rect, gradient.stops.first());
    }

    fn draw_text(&mut self, text: &str, rect: Rect, style: &TextStyle) {
        let rect = self.rect(rect);
        super::text::draw(
            self.canvas,
            text,
            rect,
            style,
            self.dpi,
            Self::text_format(style),
        );
    }

    fn draw_image(&mut self, image: &Image, rect: Rect) {
        let rect = self.rect(rect);
        if rect.is_empty() {
            return;
        }
        // Reuse the platform layer's RGBA buffer and WIC resampler so the
        // DIB's per-pixel alpha matches what the rest of the crate uploads.
        // Uploading allocates a DIB (and a WIC resample when scaling); an
        // image cache would remove that, but an image is not a per-item draw.
        let rgba = crate::RgbaImage {
            width: image.width(),
            height: image.height(),
            pixels: image.pixels().to_vec(),
        };
        let target = (rect.width().max(1) as u32, rect.height().max(1) as u32);
        let rgba = if (rgba.width, rgba.height) == target {
            rgba
        } else {
            match crate::imaging::resize(&rgba, target.0, target.1) {
                Ok(scaled) => scaled,
                Err(_) => return,
            }
        };
        let Ok(bitmap) =
            gdi::Bitmap::from_rgba(rgba.width as i32, rgba.height as i32, &rgba.pixels)
        else {
            return;
        };
        self.canvas.draw_bitmap(&bitmap, rect);
    }

    fn push_clip(&mut self, rect: Rect) {
        let rect = self.rect(rect);
        let clipped = self
            .clips
            .last()
            .map_or(rect, |outer| intersect(outer.bounds, rect));
        self.clips.push(Clip {
            bounds: clipped,
            corners: None,
        });
    }

    fn push_clip_rounded(&mut self, rect: Rect, corners: [Corner; 4]) {
        let rect = self.rect(rect);
        let clipped = self
            .clips
            .last()
            .map_or(rect, |outer| intersect(outer.bounds, rect));
        self.clips.push(Clip {
            bounds: clipped,
            corners: Some(corners),
        });
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
