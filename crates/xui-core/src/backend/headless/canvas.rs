#![forbid(unsafe_code)]

//! The headless backend's recording canvas and its draw-op log.

use crate::backend::canvas::{Canvas, TextStyle};
use crate::color::Color;
use crate::geometry::{Point, Rect};
/// One drawing command a painted node recorded.
#[derive(Clone, Debug, PartialEq)]
pub enum DrawOp {
    /// Fills the whole surface.
    Clear(Color),
    /// Fills a rectangle.
    Fill(Rect, Color),
    /// Fills a rounded rectangle.
    Rounded(Rect, f32, Color),
    /// Fills an ellipse.
    Ellipse(Point, f32, f32, Color),
    /// Strokes a rectangle.
    Stroke(Rect, Color, f32),
    /// Strokes a rounded rectangle.
    StrokeRounded(Rect, f32, Color, f32),
    /// Strokes an ellipse.
    StrokeEllipse(Point, f32, f32, Color, f32),
    /// Draws a line.
    Line(Point, Point, Color, f32),
    /// Draws text.
    Text(Rect, String, Color),
    /// Pushes a clip.
    Clip(Rect),
    /// Pops a clip.
    Unclip,
}

/// Records the calls a painted node makes, applying its transform.
pub(crate) struct RecordingCanvas {
    pub(crate) ops: Vec<DrawOp>,
    tx: f32,
    ty: f32,
    scale: f32,
    clip: Vec<Rect>,
    saved: Vec<(f32, f32, f32)>,
    bounds: Rect,
    dpi: u32,
}

impl RecordingCanvas {
    pub(crate) fn new(bounds: Rect, dpi: u32) -> RecordingCanvas {
        RecordingCanvas {
            ops: Vec::new(),
            tx: 0.0,
            ty: 0.0,
            scale: 1.0,
            clip: Vec::new(),
            saved: Vec::new(),
            bounds,
            dpi,
        }
    }

    fn point(&self, point: Point) -> Point {
        Point::new(
            (self.tx + point.x as f32 * self.scale).round() as i32,
            (self.ty + point.y as f32 * self.scale).round() as i32,
        )
    }

    fn rect(&self, rect: Rect) -> Rect {
        Rect::new(
            (self.tx + rect.left as f32 * self.scale).round() as i32,
            (self.ty + rect.top as f32 * self.scale).round() as i32,
            (self.tx + rect.right as f32 * self.scale).round() as i32,
            (self.ty + rect.bottom as f32 * self.scale).round() as i32,
        )
    }
}

impl Canvas for RecordingCanvas {
    fn dpi(&self) -> u32 {
        self.dpi
    }

    fn bounds(&self) -> Rect {
        self.bounds
    }

    fn clear(&mut self, color: Color) {
        self.ops.push(DrawOp::Clear(color));
    }

    fn fill_rect(&mut self, rect: Rect, color: Color) {
        let rect = self.rect(rect);
        self.ops.push(DrawOp::Fill(rect, color));
    }

    fn fill_rounded_rect(&mut self, rect: Rect, radius: f32, color: Color) {
        let rect = self.rect(rect);
        self.ops
            .push(DrawOp::Rounded(rect, radius * self.scale, color));
    }

    fn fill_ellipse(&mut self, center: Point, radius_x: f32, radius_y: f32, color: Color) {
        let center = self.point(center);
        self.ops.push(DrawOp::Ellipse(
            center,
            radius_x * self.scale,
            radius_y * self.scale,
            color,
        ));
    }

    fn stroke_rect(&mut self, rect: Rect, color: Color, width: f32) {
        let rect = self.rect(rect);
        self.ops.push(DrawOp::Stroke(rect, color, width));
    }

    fn stroke_rounded_rect(&mut self, rect: Rect, radius: f32, color: Color, width: f32) {
        let rect = self.rect(rect);
        self.ops.push(DrawOp::StrokeRounded(
            rect,
            radius * self.scale,
            color,
            width,
        ));
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
        self.ops.push(DrawOp::StrokeEllipse(
            center,
            radius_x * self.scale,
            radius_y * self.scale,
            color,
            width,
        ));
    }

    fn draw_line(&mut self, from: Point, to: Point, color: Color, width: f32) {
        let (from, to) = (self.point(from), self.point(to));
        self.ops.push(DrawOp::Line(from, to, color, width));
    }

    fn draw_text(&mut self, text: &str, rect: Rect, style: &TextStyle) {
        let rect = self.rect(rect);
        self.ops
            .push(DrawOp::Text(rect, text.to_string(), style.color));
    }

    fn push_clip(&mut self, rect: Rect) {
        let rect = self.rect(rect);
        let clipped = self
            .clip
            .last()
            .map_or(rect, |outer| intersect(*outer, rect));
        self.clip.push(clipped);
        self.ops.push(DrawOp::Clip(clipped));
    }

    fn pop_clip(&mut self) {
        self.clip.pop();
        self.ops.push(DrawOp::Unclip);
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

fn intersect(a: Rect, b: Rect) -> Rect {
    Rect::new(
        a.left.max(b.left),
        a.top.max(b.top),
        a.right.min(b.right),
        a.bottom.min(b.bottom),
    )
}
