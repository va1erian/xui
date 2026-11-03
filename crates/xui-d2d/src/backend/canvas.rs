#![forbid(unsafe_code)]

//! The portable [`Canvas`] over a Direct2D [`Target`].
//!
//! A canvas is one node's view of the frame: the target's origin is the node's
//! top-left, and the canvas maps the node's own translation and scale itself,
//! rounding rectangles to whole pixels as the software canvas does, so the two
//! backends place a widget identically.

use std::rc::Rc;

use xui_core::backend::{
    Canvas, Corner, LinearGradient, PathGradient, PathPlacement, PathSeg, RadialGradient, Rgba,
    Stroke, TextLayout, TextMetrics, TextStyle,
};
use xui_core::color::Color;
use xui_core::geometry::{Point, Rect};
use xui_core::image::Image;

use crate::sys::gfx::{DwLayout, Geometry, RectF, Target, TextParams, measure};

mod paths;

/// What a pushed clip needs to undo it.
enum Clip {
    Rect,
    Layer,
}

pub(crate) struct D2dCanvas {
    target: Rc<Target>,
    bounds: Rect,
    dpi: u32,
    over_parents: bool,
    tx: f32,
    ty: f32,
    scale: f32,
    saved: Vec<(f32, f32, f32)>,
    clips: Vec<Clip>,
}

impl D2dCanvas {
    /// A canvas for a node of `bounds` size. The caller has already set the
    /// target's origin to the node's top-left.
    pub(crate) fn new(target: Rc<Target>, bounds: Rect, dpi: u32, over_parents: bool) -> D2dCanvas {
        D2dCanvas {
            target,
            bounds,
            dpi,
            over_parents,
            tx: 0.0,
            ty: 0.0,
            scale: 1.0,
            saved: Vec::new(),
            clips: Vec::new(),
        }
    }

    fn map(&self, value: f32, offset: f32) -> f32 {
        (offset + value * self.scale).round()
    }

    fn rect(&self, rect: Rect) -> RectF {
        RectF::new(
            self.map(rect.left as f32, self.tx),
            self.map(rect.top as f32, self.ty),
            self.map(rect.right as f32, self.tx),
            self.map(rect.bottom as f32, self.ty),
        )
    }

    fn point(&self, point: Point) -> (f32, f32) {
        (
            self.map(point.x as f32, self.tx),
            self.map(point.y as f32, self.ty),
        )
    }

    fn corners(&self, corners: [Corner; 4]) -> [Corner; 4] {
        corners.map(|c| Corner::new(c.x * self.scale, c.y * self.scale))
    }

    fn params<'a>(&self, style: &'a TextStyle) -> TextParams<'a> {
        TextParams {
            family: style.family.as_deref(),
            size_px: style.size.to_px(self.dpi).value() as f32,
            weight: style.weight.value(),
            italic: style.italic,
            align: style.align,
            valign: style.valign,
            wrap: style.wrap,
        }
    }
}

fn opaque(color: Color) -> Rgba {
    color.into()
}

impl Canvas for D2dCanvas {
    fn dpi(&self) -> u32 {
        self.dpi
    }

    fn bounds(&self) -> Rect {
        self.bounds
    }

    fn composites_parents(&self) -> bool {
        self.over_parents
    }

    fn clear(&mut self, color: Color) {
        self.fill_rect(self.bounds, color);
    }

    fn fill_rect(&mut self, rect: Rect, color: Color) {
        self.target.fill_rect(self.rect(rect), opaque(color));
    }

    fn fill_rounded_rect(&mut self, rect: Rect, radius: f32, color: Color) {
        let radius = radius * self.scale;
        self.target
            .fill_round_rect(self.rect(rect), radius, opaque(color));
    }

    fn fill_ellipse(&mut self, center: Point, radius_x: f32, radius_y: f32, color: Color) {
        let radii = (radius_x * self.scale, radius_y * self.scale);
        self.target
            .fill_ellipse(self.point(center), radii, opaque(color));
    }

    fn fill_polygon(&mut self, points: &[Point], color: Color) {
        if points.len() < 3 {
            return;
        }
        let mapped: Vec<(f32, f32)> = points.iter().map(|p| self.point(*p)).collect();
        if let Some(geometry) = Geometry::polygon(&mapped) {
            self.target.fill_geometry(&geometry, opaque(color));
        }
    }

    fn stroke_rect(&mut self, rect: Rect, color: Color, width: f32) {
        self.target
            .stroke_rect(self.rect(rect), opaque(color), width.max(1.0));
    }

    fn stroke_rounded_rect(&mut self, rect: Rect, radius: f32, color: Color, width: f32) {
        let radius = radius * self.scale;
        self.target
            .stroke_round_rect(self.rect(rect), radius, opaque(color), width.max(1.0));
    }

    fn stroke_ellipse(&mut self, center: Point, rx: f32, ry: f32, color: Color, width: f32) {
        let radii = (rx * self.scale, ry * self.scale);
        self.target.stroke_ellipse(
            self.point(center),
            radii,
            opaque(color),
            &Stroke::new(width.max(1.0)),
        );
    }

    fn draw_line(&mut self, from: Point, to: Point, color: Color, width: f32) {
        self.draw_line_stroked(from, to, opaque(color), &Stroke::new(width));
    }

    fn fill_rect_rgba(&mut self, rect: Rect, color: Rgba) {
        self.target.fill_rect(self.rect(rect), color);
    }

    fn fill_rounded_rect_corners(&mut self, rect: Rect, corners: [Corner; 4], color: Rgba) {
        if let Some(geometry) = Geometry::rounded_rect(self.rect(rect), self.corners(corners)) {
            self.target.fill_geometry(&geometry, color);
        }
    }

    fn stroke_rounded_rect_corners(
        &mut self,
        rect: Rect,
        corners: [Corner; 4],
        color: Rgba,
        stroke: &Stroke,
    ) {
        if let Some(geometry) = Geometry::rounded_rect(self.rect(rect), self.corners(corners)) {
            self.target.stroke_geometry(&geometry, color, stroke);
        }
    }

    fn draw_line_stroked(&mut self, from: Point, to: Point, color: Rgba, stroke: &Stroke) {
        let (from, to) = (self.point(from), self.point(to));
        self.target.line(from, to, color, stroke);
    }

    fn stroke_ellipse_stroked(
        &mut self,
        center: Point,
        radius_x: f32,
        radius_y: f32,
        color: Rgba,
        stroke: &Stroke,
    ) {
        let radii = (radius_x * self.scale, radius_y * self.scale);
        self.target
            .stroke_ellipse(self.point(center), radii, color, stroke);
    }

    fn fill_path(&mut self, path: &[PathSeg], at: PathPlacement, color: Rgba) {
        if let Some(geometry) = self.geometry(path, at) {
            self.target.fill_geometry(&geometry, color);
        }
    }

    fn fill_path_linear(&mut self, path: &[PathSeg], at: PathPlacement, gradient: &PathGradient) {
        if let Some(geometry) = self.geometry(path, at) {
            let (start, end) = (
                self.path_point(gradient.start, at),
                self.path_point(gradient.end, at),
            );
            self.target
                .fill_geometry_linear(&geometry, start, end, gradient.stops);
        }
    }

    fn stroke_path(&mut self, path: &[PathSeg], at: PathPlacement, color: Rgba, stroke: &Stroke) {
        if let Some(geometry) = self.geometry(path, at) {
            self.target.stroke_geometry(&geometry, color, stroke);
        }
    }

    fn fill_rect_linear(&mut self, rect: Rect, gradient: &LinearGradient) {
        self.target.fill_rect_linear(
            self.rect(rect),
            self.point(gradient.start),
            self.point(gradient.end),
            &gradient.stops,
        );
    }

    fn fill_rect_radial(&mut self, rect: Rect, gradient: &RadialGradient) {
        let radii = (
            gradient.radius_x * self.scale,
            gradient.radius_y * self.scale,
        );
        self.target.fill_rect_radial(
            self.rect(rect),
            self.point(gradient.center),
            radii,
            &gradient.stops,
        );
    }

    fn draw_text(&mut self, text: &str, rect: Rect, style: &TextStyle) {
        let color = opaque(style.color);
        self.target
            .draw_text(text, self.rect(rect), &self.params(style), color);
    }

    fn measure_text(&self, text: &str, style: &TextStyle) -> TextMetrics {
        metrics(text, &self.params(style))
    }

    fn draw_layout(&mut self, layout: &dyn TextLayout, origin: Point, color: Rgba) {
        if let Some(layout) = layout.as_any().downcast_ref::<DwLayout>() {
            self.target.draw_layout(layout, self.point(origin), color);
        }
    }

    fn draw_image(&mut self, image: &Image, rect: Rect) {
        self.target.draw_image(
            image.id(),
            image.width(),
            image.height(),
            image.pixels(),
            self.rect(rect),
        );
    }

    fn push_clip(&mut self, rect: Rect) {
        self.target.push_clip(self.rect(rect));
        self.clips.push(Clip::Rect);
    }

    fn push_clip_rounded(&mut self, rect: Rect, corners: [Corner; 4]) {
        match Geometry::rounded_rect(self.rect(rect), self.corners(corners)) {
            Some(geometry) => {
                self.target.push_clip_geometry(&geometry);
                self.clips.push(Clip::Layer);
            }
            None => self.push_clip(rect),
        }
    }

    fn pop_clip(&mut self) {
        match self.clips.pop() {
            Some(Clip::Rect) => self.target.pop_clip(),
            Some(Clip::Layer) => self.target.pop_layer(),
            None => {}
        }
    }

    fn save(&mut self) {
        self.saved.push((self.tx, self.ty, self.scale));
    }

    fn restore(&mut self) {
        if let Some((tx, ty, scale)) = self.saved.pop() {
            (self.tx, self.ty, self.scale) = (tx, ty, scale);
        }
    }

    fn set_translation(&mut self, x: f32, y: f32) {
        (self.tx, self.ty) = (x, y);
    }

    fn set_scale_translate(&mut self, scale: f32, x: f32, y: f32) {
        (self.scale, self.tx, self.ty) = (scale, x, y);
    }
}

impl Drop for D2dCanvas {
    /// Pops any clip a painter left pushed, so the frame's clip stack stays
    /// balanced for the next node.
    fn drop(&mut self) {
        while !self.clips.is_empty() {
            self.pop_clip();
        }
    }
}

/// Measures `text` into the portable integer metrics.
pub(super) fn metrics(text: &str, params: &TextParams<'_>) -> TextMetrics {
    let (width, height, baseline) = measure(text, params);
    TextMetrics {
        width: width.ceil() as i32,
        height: height.ceil() as i32,
        ascent: baseline.round() as i32,
        descent: (height - baseline).round().max(0.0) as i32,
    }
}
