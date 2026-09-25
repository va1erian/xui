#![forbid(unsafe_code)]

//! The portable drawing surface a [`Painted`](super::ImplKind::Painted) widget
//! draws into. A backend implements it over whatever it rasterizes with
//! (Direct2D, tiny-skia, …).
//!
//! Coordinates are device pixels unless a method says otherwise; the front
//! layer converts its [`Dip`] design values once, at the boundary.

use crate::color::Color;
use crate::geometry::{Point, Rect};
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
            wrap: false,
        }
    }

    /// Uses a heavier face.
    pub fn bold(mut self) -> TextStyle {
        self.weight = TextWeight::Bold;
        self
    }

    /// Centres the run.
    pub fn centered(mut self) -> TextStyle {
        self.align = TextAlign::Center;
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

    /// Draws `text` inside `rect` using `style`.
    fn draw_text(&mut self, text: &str, rect: Rect, style: &TextStyle);

    /// Clips subsequent drawing to `rect` until the matching `pop_clip`.
    ///
    /// Saves and restores the *clip only*, not the transform; wrap a nested
    /// draw in [`Canvas::save`]/[`Canvas::restore`] to restore both.
    fn push_clip(&mut self, rect: Rect);

    /// Removes the most recent clip.
    fn pop_clip(&mut self);

    /// Saves the whole graphics state (clip, transform and scale) on a stack.
    fn save(&mut self);

    /// Restores the state saved by the most recent [`Canvas::save`].
    fn restore(&mut self);

    /// Replaces the current translation (it does not compose with a previous
    /// one).
    fn set_translation(&mut self, x: f32, y: f32);

    /// Replaces the current uniform scale and translation.
    fn set_scale_translate(&mut self, scale: f32, x: f32, y: f32);
}
