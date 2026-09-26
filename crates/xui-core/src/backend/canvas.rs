#![forbid(unsafe_code)]

//! The portable drawing surface a [`Painted`](super::ImplKind::Painted) widget
//! draws into. A backend implements it over whatever it rasterizes with
//! (Direct2D, tiny-skia, …).
//!
//! Coordinates are device pixels unless a method says otherwise; the front
//! layer converts its [`Dip`] design values once, at the boundary.

use super::paint::{Corner, LinearGradient, RadialGradient, Rgba, Stroke};
use crate::color::Color;
use crate::geometry::{Point, Rect};
use crate::image::Image;
use crate::units::Dip;

/// How text is aligned inside the rectangle it is drawn into.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TextAlign {
    /// Left-aligned (the default reading order start).
    #[default]
    Start,
    /// Centred.
    Center,
    /// Right-aligned (the reading order end).
    End,
}

/// Vertical placement of a text run inside its rectangle.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TextVAlign {
    /// Aligned to the top (the default).
    #[default]
    Top,
    /// Vertically centred.
    Middle,
}

/// A text weight, coarse enough for every backend to honour.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TextWeight {
    /// The regular face (the default).
    #[default]
    Regular,
    /// A heavier face.
    Bold,
}

/// How to draw a run of text.
#[derive(Clone, Debug, PartialEq)]
pub struct TextStyle {
    /// The text colour.
    pub color: Color,
    /// The font size as a design value.
    pub size: Dip,
    /// The face weight.
    pub weight: TextWeight,
    /// The horizontal alignment inside the target rectangle.
    pub align: TextAlign,
    /// The vertical placement inside the target rectangle.
    pub valign: TextVAlign,
    /// Whether to wrap across lines when the rectangle is too narrow.
    pub wrap: bool,
}

impl TextStyle {
    /// A left-aligned, non-wrapping run of `size` in `color`.
    pub fn new(color: Color, size: Dip) -> TextStyle {
        TextStyle {
            color,
            size,
            weight: TextWeight::Regular,
            align: TextAlign::Start,
            valign: TextVAlign::Top,
            wrap: false,
        }
    }

    /// Uses a heavier face.
    pub fn bold(mut self) -> TextStyle {
        self.weight = TextWeight::Bold;
        self
    }

    /// Centres the run horizontally.
    pub fn centered(mut self) -> TextStyle {
        self.align = TextAlign::Center;
        self
    }

    /// Centres the run vertically.
    pub fn middle(mut self) -> TextStyle {
        self.valign = TextVAlign::Middle;
        self
    }

    /// Wraps the run across lines.
    pub fn wrapped(mut self) -> TextStyle {
        self.wrap = true;
        self
    }
}

/// The measured size of a run of text, in device pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TextMetrics {
    /// The advance width.
    pub width: i32,
    /// The line height for a single line.
    pub height: i32,
    /// The distance from the top to the baseline.
    pub ascent: i32,
    /// The distance from the baseline to the bottom.
    pub descent: i32,
}

/// A drawing surface for a painted widget.
///
/// Coordinates are in the widget's client pixels, with the origin at its
/// top-left. The surface is short-lived: it is only valid for the duration of
/// the paint callback it is handed to, and a widget must not retain it.
pub trait Canvas {
    /// The surface's dots-per-inch, for a widget that converts design values
    /// itself.
    fn dpi(&self) -> u32;

    /// The drawable area, at the origin.
    fn bounds(&self) -> Rect;

    /// Fills the surface with `color`.
    fn clear(&mut self, color: Color);

    /// Fills an axis-aligned rectangle.
    fn fill_rect(&mut self, rect: Rect, color: Color);

    /// Fills a rectangle with rounded corners of `radius` pixels.
    fn fill_rounded_rect(&mut self, rect: Rect, radius: f32, color: Color);

    /// Fills an ellipse centred on `center`.
    fn fill_ellipse(&mut self, center: Point, radius_x: f32, radius_y: f32, color: Color);

    /// Fills the polygon through `points` (at least three), in the canvas's own
    /// coordinates, applying the current transform and clip like a shape.
    fn fill_polygon(&mut self, points: &[Point], color: Color);

    /// Strokes a rectangle outline of `width` pixels.
    fn stroke_rect(&mut self, rect: Rect, color: Color, width: f32);

    /// Strokes a rounded rectangle outline.
    fn stroke_rounded_rect(&mut self, rect: Rect, radius: f32, color: Color, width: f32);

    /// Strokes an ellipse outline.
    fn stroke_ellipse(
        &mut self,
        center: Point,
        radius_x: f32,
        radius_y: f32,
        color: Color,
        width: f32,
    );

    /// Draws a straight line.
    fn draw_line(&mut self, from: Point, to: Point, color: Color, width: f32);

    /// Fills an axis-aligned rectangle with an RGBA colour, alpha included.
    fn fill_rect_rgba(&mut self, rect: Rect, color: Rgba);

    /// Fills a rectangle whose corners carry their own elliptical radii. The
    /// corners are ordered top-left, top-right, bottom-right, bottom-left.
    fn fill_rounded_rect_corners(&mut self, rect: Rect, corners: [Corner; 4], color: Rgba);

    /// Strokes a rectangle whose corners carry their own elliptical radii.
    fn stroke_rounded_rect_corners(
        &mut self,
        rect: Rect,
        corners: [Corner; 4],
        color: Rgba,
        stroke: &Stroke,
    );

    /// Draws a line with the full stroke vocabulary (width, dash, cap) and an
    /// RGBA colour.
    ///
    /// This is the RGBA/dashed counterpart of [`Canvas::draw_line`]; a trait
    /// cannot overload a method by its argument types, so the two carry
    /// different names.
    fn draw_line_stroked(&mut self, from: Point, to: Point, color: Rgba, stroke: &Stroke);

    /// Strokes an ellipse outline with the full stroke vocabulary.
    ///
    /// The RGBA/dashed counterpart of [`Canvas::stroke_ellipse`].
    fn stroke_ellipse_stroked(
        &mut self,
        center: Point,
        radius_x: f32,
        radius_y: f32,
        color: Rgba,
        stroke: &Stroke,
    );

    /// Fills an axis-aligned rectangle with a linear gradient.
    fn fill_rect_linear(&mut self, rect: Rect, gradient: &LinearGradient);

    /// Fills an axis-aligned rectangle with a radial gradient.
    fn fill_rect_radial(&mut self, rect: Rect, gradient: &RadialGradient);

    /// Draws `text` inside `rect` using `style`.
    fn draw_text(&mut self, text: &str, rect: Rect, style: &TextStyle);

    /// Draws `image` scaled into `rect`, honouring the current clip. The image
    /// fills the rectangle exactly; its aspect ratio is not preserved.
    fn draw_image(&mut self, image: &Image, rect: Rect);

    /// Clips subsequent drawing to `rect` until the matching `pop_clip`.
    ///
    /// `push_clip`/`pop_clip` manage only the clip; the transform is untouched.
    /// [`Canvas::save`]/[`Canvas::restore`] manage only the transform and
    /// scale; the clip is untouched. Pop every clip you push and restore every
    /// save you make, in order.
    fn push_clip(&mut self, rect: Rect);

    /// Clips subsequent drawing to the rounded rectangle `rect`/`corners`,
    /// with per-corner elliptical radii, until the matching
    /// [`Canvas::pop_clip`].
    ///
    /// Uses the same clip stack as [`Canvas::push_clip`], so a rounded clip and
    /// an axis-aligned one may nest freely.
    fn push_clip_rounded(&mut self, rect: Rect, corners: [Corner; 4]);

    /// Removes the most recent clip.
    fn pop_clip(&mut self);

    /// Saves the transform and scale on a stack. Does not save the clip.
    fn save(&mut self);

    /// Restores the transform and scale saved by the most recent
    /// [`Canvas::save`].
    fn restore(&mut self);

    /// Replaces the current translation (it does not compose with a previous
    /// one).
    fn set_translation(&mut self, x: f32, y: f32);

    /// Replaces the current uniform scale and translation.
    fn set_scale_translate(&mut self, scale: f32, x: f32, y: f32);
}
