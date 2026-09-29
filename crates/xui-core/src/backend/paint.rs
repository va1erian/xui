#![forbid(unsafe_code)]

//! The portable shape vocabulary a display-list command maps onto: an RGBA
//! colour, per-corner elliptical radii, the stroke (width, dash, cap)
//! description and the two gradient definitions.
//!
//! Each backend maps these onto its own drawing calls. They live beside the
//! [`Canvas`](super::Canvas) trait rather than in it so that file can hold the
//! trait alone.

use crate::color::Color;
use crate::geometry::Point;

/// A colour with an alpha channel, for a fill or stroke that blends.
///
/// [`Color`] is opaque, so it can be a plain theme token; the alpha a blending
/// token needs (the modal scrim) and a recorded fill that must blend carry it
/// here. The alpha is straight (not premultiplied).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Rgba {
    /// Red channel.
    pub r: u8,
    /// Green channel.
    pub g: u8,
    /// Blue channel.
    pub b: u8,
    /// Alpha: `0` is fully transparent, `255` fully opaque.
    pub a: u8,
}

impl Rgba {
    /// An opaque colour from its channels.
    pub const fn rgb(r: u8, g: u8, b: u8) -> Rgba {
        Rgba { r, g, b, a: 0xFF }
    }

    /// A colour from its channels and alpha.
    pub const fn with_alpha(r: u8, g: u8, b: u8, a: u8) -> Rgba {
        Rgba { r, g, b, a }
    }

    /// Fully transparent black.
    pub const TRANSPARENT: Rgba = Rgba::with_alpha(0x00, 0x00, 0x00, 0x00);
}

impl From<Color> for Rgba {
    fn from(color: Color) -> Rgba {
        Rgba::rgb(color.r, color.g, color.b)
    }
}

/// An elliptical corner radius: an x and a y half-axis, as CSS
/// `border-radius` describes one.
///
/// `Corner::uniform(r)` is the circular case; `Corner::new(rx, ry)` gives an
/// elliptical corner. The corners of a rounded rectangle are ordered top-left,
/// top-right, bottom-right, bottom-left.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Corner {
    /// The horizontal half-axis.
    pub x: f32,
    /// The vertical half-axis.
    pub y: f32,
}

impl Corner {
    /// A corner with equal x and y half-axes.
    pub const fn uniform(radius: f32) -> Corner {
        Corner {
            x: radius,
            y: radius,
        }
    }

    /// A corner with separate x and y half-axes.
    pub const fn new(x: f32, y: f32) -> Corner {
        Corner { x, y }
    }
}

/// How a stroked line or outline is broken up.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Dash {
    /// An unbroken line.
    #[default]
    Solid,
    /// Dashes.
    Dashed,
    /// Dots.
    Dotted,
}

/// The shape of a stroke's endpoints and dash ends.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Cap {
    /// A flat end that stops at the endpoint.
    #[default]
    Flat,
    /// A squared end that extends half the width past the endpoint.
    Square,
    /// A rounded end; usually wanted for dotted lines.
    Round,
}

/// How two stroked segments meet at a corner.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Join {
    /// A sharp corner.
    #[default]
    Miter,
    /// A flat, chamfered corner.
    Bevel,
    /// A rounded corner.
    Round,
}

/// A stroked outline: a width, a dash pattern, a cap shape and a join shape.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stroke {
    /// The line width.
    pub width: f32,
    /// The dash pattern.
    pub dash: Dash,
    /// The endpoint and dash-end shape.
    pub cap: Cap,
    /// The corner shape of a stroked path.
    pub join: Join,
}

impl Stroke {
    /// A solid stroke `width` wide with flat caps.
    pub const fn new(width: f32) -> Stroke {
        Stroke {
            width,
            dash: Dash::Solid,
            cap: Cap::Flat,
            join: Join::Miter,
        }
    }

    /// The same stroke with `dash`.
    pub const fn dash(self, dash: Dash) -> Stroke {
        Stroke { dash, ..self }
    }

    /// The same stroke with `cap`.
    pub const fn cap(self, cap: Cap) -> Stroke {
        Stroke { cap, ..self }
    }

    /// The same stroke with `join`.
    pub const fn join(self, join: Join) -> Stroke {
        Stroke { join, ..self }
    }
}

/// One colour in a gradient, at `position` in `0.0..=1.0`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GradientStop {
    /// The position along the gradient, in `0.0..=1.0`.
    pub position: f32,
    /// The colour at that position.
    pub color: Rgba,
}

impl GradientStop {
    /// A stop at `position` with `color`.
    pub const fn new(position: f32, color: Rgba) -> GradientStop {
        GradientStop { position, color }
    }
}

/// A linear gradient from `start` to `end` in canvas coordinates, defined by
/// its [`GradientStop`]s (at least two).
#[derive(Clone, Debug, PartialEq)]
pub struct LinearGradient {
    /// Where the gradient starts (the position of stop `0.0`).
    pub start: Point,
    /// Where the gradient ends (the position of stop `1.0`).
    pub end: Point,
    /// The colours at their positions.
    pub stops: Vec<GradientStop>,
}

impl LinearGradient {
    /// A gradient from `start` to `end` over `stops`.
    pub fn new(start: Point, end: Point, stops: Vec<GradientStop>) -> LinearGradient {
        LinearGradient { start, end, stops }
    }
}

/// A radial gradient centred at `center` with elliptical radii, defined by its
/// [`GradientStop`]s (at least two).
#[derive(Clone, Debug, PartialEq)]
pub struct RadialGradient {
    /// The centre of the gradient (the position of stop `0.0`).
    pub center: Point,
    /// The horizontal radius (the position of stop `1.0` along x).
    pub radius_x: f32,
    /// The vertical radius (the position of stop `1.0` along y).
    pub radius_y: f32,
    /// The colours at their positions.
    pub stops: Vec<GradientStop>,
}

impl RadialGradient {
    /// A gradient centred at `center` with radii `radius_x`×`radius_y`.
    pub fn new(
        center: Point,
        radius_x: f32,
        radius_y: f32,
        stops: Vec<GradientStop>,
    ) -> RadialGradient {
        RadialGradient {
            center,
            radius_x,
            radius_y,
            stops,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn color_converts_to_opaque_rgba() {
        let rgba: Rgba = Color::rgb(0x12, 0x34, 0x56).into();
        assert_eq!((rgba.r, rgba.g, rgba.b, rgba.a), (0x12, 0x34, 0x56, 0xFF));
    }

    #[test]
    fn stroke_builders_set_dash_and_cap() {
        let stroke = Stroke::new(2.0).dash(Dash::Dotted).cap(Cap::Round);
        assert_eq!(
            (stroke.width, stroke.dash, stroke.cap),
            (2.0, Dash::Dotted, Cap::Round)
        );
    }

    #[test]
    fn gradient_constructors_keep_their_stops() {
        let stops = vec![
            GradientStop::new(0.0, Rgba::TRANSPARENT),
            GradientStop::new(1.0, Rgba::rgb(0, 0, 0)),
        ];
        let linear = LinearGradient::new(Point::new(0, 0), Point::new(10, 0), stops.clone());
        let radial = RadialGradient::new(Point::new(5, 5), 4.0, 2.0, stops);
        assert_eq!(linear.stops.len(), 2);
        assert_eq!((radial.radius_x, radial.radius_y), (4.0, 2.0));
    }
}
