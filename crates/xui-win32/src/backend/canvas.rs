#![forbid(unsafe_code)]

//! The Win32 implementation of the portable [`Canvas`]: shapes go through the
//! GDI/Direct2D [`gdi::Canvas`], and the wrapper tracks the translation, scale
//! and clip the core contract requires.
//!
//! Clipping is applied by intersecting each shape's rectangle with the current
//! clip; it is exact for the axis-aligned rectangles a widget draws, and a
//! stroke that straddles the clip edge is not trimmed.

use crate::d2d::{PointF, RectF, Stroke};
use crate::gdi::{self, TextFormat};
use xui_core::backend::{Canvas, TextAlign, TextStyle, TextVAlign};
use xui_core::{Color, Point, Rect};

/// A portable canvas over a Win32 GDI [`gdi::Canvas`].
pub(crate) struct Win32Canvas<'a> {
    canvas: &'a gdi::Canvas,
    bounds: Rect,
    dpi: u32,
    tx: f32,
    ty: f32,
    scale: f32,
    clips: Vec<Rect>,
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

    fn text_format(style: &TextStyle) -> TextFormat {
        let format = match style.align {
            TextAlign::Start => TextFormat::left(),
            TextAlign::Center => TextFormat::left().center(),
            TextAlign::End => TextFormat::left().right(),
        };
        let format = match style.valign {
            TextVAlign::Top => format,
            TextVAlign::Middle => format.vcenter(),
        };
        if style.wrap {
            format.word_wrap()
        } else {
            format.single_line()
        }
    }

    fn d2d(&self) -> Option<crate::d2d::DcCanvas> {
        self.canvas.d2d()
    }

    /// The device-space bounding box of an ellipse, after the transform.
    fn ellipse_bounds(&self, center: Point, radius_x: f32, radius_y: f32) -> Rect {
        let center = self.point(center);
        let rx = radius_x * self.scale;
        let ry = radius_y * self.scale;
        Rect::new(
            (center.x as f32 - rx).round() as i32,
            (center.y as f32 - ry).round() as i32,
            (center.x as f32 + rx).round() as i32,
            (center.y as f32 + ry).round() as i32,
        )
    }

    /// Whether an ellipse is entirely outside the current clip.
    fn ellipse_clipped_out(&self, bounds: Rect) -> bool {
        self.clips
            .last()
            .is_some_and(|clip| intersect(bounds, *clip).is_empty())
    }
}

fn intersect(a: Rect, b: Rect) -> Rect {
    Rect::new(
        a.left.max(b.left),
        a.top.max(b.top),
        a.right.min(b.right),
        a.bottom.min(b.bottom),
    )
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
        if let Some(mut d2d) = self.d2d() {
            d2d.stroke_rect(RectF::from_rect(rect), color, Stroke::solid(width.max(1.0)));
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
            d2d.stroke_rounded_rect(
                RectF::from_rect(rect),
                radius.max(1.0),
                color,
                Stroke::solid(width.max(1.0)),
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
            let center = self.point(center);
            d2d.stroke_ellipse(
                PointF::new(center.x as f32, center.y as f32),
                radius_x * self.scale,
                radius_y * self.scale,
                color,
                Stroke::solid(width.max(1.0)),
            );
            let _ = d2d.end_draw();
        }
    }

    fn draw_line(&mut self, from: Point, to: Point, color: Color, width: f32) {
        let (from, to) = (self.point(from), self.point(to));
        self.canvas.line(from, to, color, width.max(1.0) as i32);
    }

    fn draw_text(&mut self, text: &str, rect: Rect, style: &TextStyle) {
        let rect = self.rect(rect);
        self.canvas
            .draw_text(rect, text, style.color, Self::text_format(style));
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
