#![forbid(unsafe_code)]

//! The headless backend's recording canvas and its draw-op log.

use crate::backend::canvas::{Canvas, TextStyle};
use crate::backend::paint::{Corner, LinearGradient, RadialGradient, Rgba, Stroke};
use crate::backend::text::TextLayout;
use crate::color::Color;
use crate::geometry::{Point, Rect};
use crate::image::Image;
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
    /// Fills a polygon through the given points.
    Polygon(Vec<Point>, Color),
    /// Strokes a rectangle.
    Stroke(Rect, Color, f32),
    /// Strokes a rounded rectangle.
    StrokeRounded(Rect, f32, Color, f32),
    /// Strokes an ellipse.
    StrokeEllipse(Point, f32, f32, Color, f32),
    /// Draws a line.
    Line(Point, Point, Color, f32),
    /// Fills a rectangle with an RGBA colour.
    FillRgba(Rect, Rgba),
    /// Fills a rectangle with per-corner elliptical radii.
    RoundedCorners(Rect, [Corner; 4], Rgba),
    /// Strokes a rectangle with per-corner elliptical radii.
    StrokeRoundedCorners(Rect, [Corner; 4], Rgba, Stroke),
    /// Draws a line with the full stroke vocabulary.
    LineStroked(Point, Point, Rgba, Stroke),
    /// Strokes an ellipse with the full stroke vocabulary.
    StrokeEllipseStroked(Point, f32, f32, Rgba, Stroke),
    /// Fills a rectangle with a linear gradient.
    FillLinear(Rect, LinearGradient),
    /// Fills a rectangle with a radial gradient.
    FillRadial(Rect, RadialGradient),
    /// Draws text.
    Text(Rect, String, Color),
    /// Draws a shaped layout at a top-left origin, in an RGBA colour.
    ShapedText(Point, Rgba),
    /// Draws an image scaled into the rectangle.
    Image(Rect, Image),
    /// Pushes a clip.
    Clip(Rect),
    /// Pushes a rounded clip.
    ClipRounded(Rect, [Corner; 4]),
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

    fn corners(&self, corners: [Corner; 4]) -> [Corner; 4] {
        corners.map(|corner| Corner::new(corner.x * self.scale, corner.y * self.scale))
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

    fn fill_polygon(&mut self, points: &[Point], color: Color) {
        let points: Vec<Point> = points.iter().map(|point| self.point(*point)).collect();
        self.ops.push(DrawOp::Polygon(points, color));
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

    fn fill_rect_rgba(&mut self, rect: Rect, color: Rgba) {
        let rect = self.rect(rect);
        self.ops.push(DrawOp::FillRgba(rect, color));
    }

    fn fill_rounded_rect_corners(&mut self, rect: Rect, corners: [Corner; 4], color: Rgba) {
        let rect = self.rect(rect);
        self.ops
            .push(DrawOp::RoundedCorners(rect, self.corners(corners), color));
    }

    fn stroke_rounded_rect_corners(
        &mut self,
        rect: Rect,
        corners: [Corner; 4],
        color: Rgba,
        stroke: &Stroke,
    ) {
        let rect = self.rect(rect);
        self.ops.push(DrawOp::StrokeRoundedCorners(
            rect,
            self.corners(corners),
            color,
            *stroke,
        ));
    }

    fn draw_line_stroked(&mut self, from: Point, to: Point, color: Rgba, stroke: &Stroke) {
        let (from, to) = (self.point(from), self.point(to));
        self.ops.push(DrawOp::LineStroked(from, to, color, *stroke));
    }

    fn stroke_ellipse_stroked(
        &mut self,
        center: Point,
        radius_x: f32,
        radius_y: f32,
        color: Rgba,
        stroke: &Stroke,
    ) {
        let center = self.point(center);
        self.ops.push(DrawOp::StrokeEllipseStroked(
            center,
            radius_x * self.scale,
            radius_y * self.scale,
            color,
            *stroke,
        ));
    }

    fn fill_rect_linear(&mut self, rect: Rect, gradient: &LinearGradient) {
        let rect = self.rect(rect);
        let gradient = LinearGradient::new(
            self.point(gradient.start),
            self.point(gradient.end),
            gradient.stops.clone(),
        );
        self.ops.push(DrawOp::FillLinear(rect, gradient));
    }

    fn fill_rect_radial(&mut self, rect: Rect, gradient: &RadialGradient) {
        let rect = self.rect(rect);
        let gradient = RadialGradient::new(
            self.point(gradient.center),
            gradient.radius_x * self.scale,
            gradient.radius_y * self.scale,
            gradient.stops.clone(),
        );
        self.ops.push(DrawOp::FillRadial(rect, gradient));
    }

    fn draw_text(&mut self, text: &str, rect: Rect, style: &TextStyle) {
        let rect = self.rect(rect);
        self.ops
            .push(DrawOp::Text(rect, text.to_string(), style.color));
    }

    fn draw_layout(&mut self, _layout: &dyn TextLayout, origin: Point, color: Rgba) {
        let origin = self.point(origin);
        self.ops.push(DrawOp::ShapedText(origin, color));
    }

    fn draw_image(&mut self, image: &Image, rect: Rect) {
        let rect = self.rect(rect);
        self.ops.push(DrawOp::Image(rect, image.clone()));
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

    fn push_clip_rounded(&mut self, rect: Rect, corners: [Corner; 4]) {
        let rect = self.rect(rect);
        let clipped = self
            .clip
            .last()
            .map_or(rect, |outer| intersect(*outer, rect));
        self.clip.push(clipped);
        self.ops
            .push(DrawOp::ClipRounded(clipped, self.corners(corners)));
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
