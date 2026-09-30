#![forbid(unsafe_code)]

//! One filled and/or stroked path of an icon.

use xui_core::backend::PathSeg;

use crate::gradient::Gradient;
use crate::tone::Tone;
use xui_core::backend::Rgba;

/// What a fill or stroke is painted with.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Paint {
    /// A [`Tone`] role, coloured by the [`Palette`](crate::Palette).
    Tone(Tone),
    /// A fixed colour, alpha included.
    Color(Rgba),
    /// A linear gradient across the shape's bounding box. Only fills use the
    /// gradient; a stroke painted with one takes its middle stop.
    Gradient(&'static Gradient),
}

/// A stroke: a paint and a width on the 32-unit grid.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Line {
    /// What the stroke is painted with.
    pub paint: Paint,
    /// The width in grid units.
    pub width: f32,
}

/// One path of an icon, with the fill and stroke it is painted with.
///
/// The path is in grid units (see [`GRID`](crate::GRID)); it is filled first
/// and stroked over the fill, like an SVG element with both set.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shape {
    /// The outline, in grid units.
    pub path: &'static [PathSeg],
    /// The fill, if the path is filled.
    pub fill: Option<Paint>,
    /// The stroke, if the path is outlined.
    pub line: Option<Line>,
    /// How opaque the whole shape is, `255` being fully.
    pub opacity: u8,
}

impl Shape {
    /// A filled path with no outline.
    pub const fn fill(path: &'static [PathSeg], fill: Paint) -> Shape {
        Shape {
            path,
            fill: Some(fill),
            line: None,
            opacity: 255,
        }
    }

    /// An outline with no fill.
    pub const fn line(path: &'static [PathSeg], paint: Paint, width: f32) -> Shape {
        Shape {
            path,
            fill: None,
            line: Some(Line { paint, width }),
            opacity: 255,
        }
    }

    /// A filled path with an outline.
    pub const fn both(path: &'static [PathSeg], fill: Paint, paint: Paint, width: f32) -> Shape {
        Shape {
            path,
            fill: Some(fill),
            line: Some(Line { paint, width }),
            opacity: 255,
        }
    }

    /// This shape at `opacity` (`255` is fully opaque).
    #[must_use]
    pub const fn opacity(mut self, opacity: u8) -> Shape {
        self.opacity = opacity;
        self
    }
}
