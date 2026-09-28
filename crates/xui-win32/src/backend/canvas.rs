#![forbid(unsafe_code)]

//! The Win32 implementation of the portable [`Canvas`]: shapes go through the
//! GDI/Direct2D [`gdi::Canvas`], and the wrapper tracks the translation, scale
//! and clip the core contract requires.
//!
//! The Direct2D frame is opened once for the whole painter and every shape,
//! text run and image draws into it, so a node costs one
//! `BeginDraw`/`EndDraw` instead of one per primitive. An axis-aligned clip is
//! applied both by intersecting each shape's rectangle with the current clip
//! (exact for the rectangles a widget draws) and, while the frame is open, as a
//! Direct2D clip, so text is clipped too. A rounded clip is a Direct2D layer.
//! Without Direct2D the whole painter falls back to GDI for good, including
//! images, which then pay a WIC resample and a DIB per draw.

use crate::d2d::{DcCanvas, PointF, RectF, Stroke as D2dStroke};
use crate::gdi;
use xui_core::backend::{
    Canvas, Corner, LinearGradient, RadialGradient, Rgba, Stroke, TextLayout, TextMetrics,
    TextStyle,
};
use xui_core::image::Image;
use xui_core::{Color, Point, Rect};

use super::shape::{d2d_stroke, intersect, linear, point_f, radial, rgba, rounded};

mod draw;

/// A portable canvas over a Win32 GDI [`gdi::Canvas`].
pub(crate) struct Win32Canvas<'a> {
    pub(crate) canvas: &'a gdi::Canvas,
    bounds: Rect,
    dpi: u32,
    pub(crate) tx: f32,
    pub(crate) ty: f32,
    pub(crate) scale: f32,
    /// The clip stack in device-space bounds, for culling and the exact
    /// intersection of an axis-aligned shape; the Direct2D frame carries the
    /// same clips itself, rounded ones included.
    pub(crate) clips: Vec<Rect>,
    saved: Vec<(f32, f32, f32)>,
    /// The Direct2D frame shared by every primitive in this painter, or `None`
    /// when Direct2D is unavailable (GDI fallback).
    pub(crate) frame: Option<DcCanvas>,
}

impl<'a> Win32Canvas<'a> {
    pub(crate) fn new(canvas: &'a gdi::Canvas, bounds: Rect, dpi: u32) -> Win32Canvas<'a> {
        // Bind and begin the Direct2D frame once; every shape, text run and
        // image in this painter reuses it. `None` when Direct2D is
        // unavailable, so all primitives fall back to GDI.
        let frame = canvas.d2d();
        Win32Canvas {
            canvas,
            bounds,
            dpi,
            tx: 0.0,
            ty: 0.0,
            scale: 1.0,
            clips: Vec::new(),
            saved: Vec::new(),
            frame,
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
        self.fill_rect(self.bounds, color);
    }

    fn fill_rect(&mut self, rect: Rect, color: Color) {
        let rect = self.rect(rect);
        if rect.is_empty() {
            return;
        }
        // Draw through the shared frame when Direct2D is available, so opaque
        // fills stay ordered with the anti-aliased shapes and text and never
        // interleave with Direct2D, which does not support that inside a frame.
        if let Some(frame) = self.frame.as_mut() {
            frame.fill_rect(RectF::from_rect(rect), color);
            return;
        }
        self.canvas.fill_rect(rect, color);
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
        let center = self.point(center);
        let scale = self.scale;
        if let Some(d2d) = self.d2d() {
            d2d.fill_ellipse(
                PointF::new(center.x as f32, center.y as f32),
                radius_x * scale,
                radius_y * scale,
                color,
            );
            return;
        }
        // GDI has no anti-aliased ellipse: fill the clipped bounding rectangle.
        let rect = self
            .clips
            .last()
            .map_or(bounds, |clip| intersect(bounds, *clip));
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
        if let Some(d2d) = self.d2d() {
            d2d.stroke_rect(
                RectF::from_rect(rect),
                color,
                D2dStroke::solid(width.max(1.0)),
            );
            return;
        }
        self.canvas.outline(rect, color);
    }

    fn stroke_rounded_rect(&mut self, rect: Rect, radius: f32, color: Color, width: f32) {
        let rect = self.rect(rect);
        if rect.is_empty() {
            return;
        }
        if let Some(d2d) = self.d2d() {
            d2d.stroke_rounded_rect(
                RectF::from_rect(rect),
                radius.max(1.0),
                color,
                D2dStroke::solid(width.max(1.0)),
            );
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
        let center = self.point(center);
        let scale = self.scale;
        if let Some(d2d) = self.d2d() {
            d2d.stroke_ellipse(
                PointF::new(center.x as f32, center.y as f32),
                radius_x * scale,
                radius_y * scale,
                color,
                D2dStroke::solid(width.max(1.0)),
            );
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
        if let Some(d2d) = self.d2d() {
            d2d.fill_rect_rgba(RectF::from_rect(rect), rgba(color));
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
        if let Some(d2d) = self.d2d() {
            d2d.fill_rounded(rounded(rect, corners), rgba(color));
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
        if let Some(d2d) = self.d2d() {
            d2d.stroke_rounded(rounded(rect, corners), rgba(color), d2d_stroke(stroke));
            return;
        }
        self.canvas
            .outline(rect, Color::rgb(color.r, color.g, color.b));
    }

    fn draw_line_stroked(&mut self, from: Point, to: Point, color: Rgba, stroke: &Stroke) {
        let (from, to) = (self.point(from), self.point(to));
        if let Some(d2d) = self.d2d() {
            d2d.draw_line_rgba(point_f(from), point_f(to), rgba(color), d2d_stroke(stroke));
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
        let center = self.point(center);
        let scale = self.scale;
        if let Some(d2d) = self.d2d() {
            d2d.stroke_ellipse_rgba(
                PointF::new(center.x as f32, center.y as f32),
                radius_x * scale,
                radius_y * scale,
                rgba(color),
                d2d_stroke(stroke),
            );
        }
    }

    fn fill_rect_linear(&mut self, rect: Rect, gradient: &LinearGradient) {
        let rect = self.rect(rect);
        if rect.is_empty() {
            return;
        }
        if let Some(d2d) = self.d2d() {
            d2d.fill_rect_linear(RectF::from_rect(rect), &linear(gradient));
            return;
        }
        self.fill_first_stop(rect, gradient.stops.first());
    }

    fn fill_rect_radial(&mut self, rect: Rect, gradient: &RadialGradient) {
        let rect = self.rect(rect);
        if rect.is_empty() {
            return;
        }
        let scale = self.scale;
        if let Some(d2d) = self.d2d() {
            d2d.fill_rect_radial(RectF::from_rect(rect), &radial(gradient, scale));
            return;
        }
        self.fill_first_stop(rect, gradient.stops.first());
    }

    fn draw_text(&mut self, text: &str, rect: Rect, style: &TextStyle) {
        self.paint_text(text, rect, style);
    }

    fn measure_text(&self, text: &str, style: &TextStyle) -> TextMetrics {
        self.measure(text, style)
    }

    fn draw_layout(&mut self, layout: &dyn TextLayout, origin: Point, color: Rgba) {
        self.paint_layout(layout, origin, color);
    }

    fn draw_image(&mut self, image: &Image, rect: Rect) {
        self.paint_image(image, rect);
    }

    fn push_clip(&mut self, rect: Rect) {
        let rect = self.rect(rect);
        let clipped = self
            .clips
            .last()
            .map_or(rect, |outer| intersect(*outer, rect));
        self.clips.push(clipped);
        if let Some(frame) = self.frame.as_mut() {
            frame.push_clip(RectF::from_rect(clipped));
        }
    }

    fn push_clip_rounded(&mut self, rect: Rect, corners: [Corner; 4]) {
        let rect = self.rect(rect);
        let clipped = self
            .clips
            .last()
            .map_or(rect, |outer| intersect(*outer, rect));
        self.clips.push(clipped);
        if let Some(frame) = self.frame.as_mut() {
            let _ = frame.push_clip_rounded(rounded(clipped, corners));
        }
    }

    fn pop_clip(&mut self) {
        if self.clips.pop().is_some()
            && let Some(frame) = self.frame.as_mut()
        {
            let _ = frame.pop_clip();
        }
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
