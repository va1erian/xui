#![forbid(unsafe_code)]

//! Gradients an icon shape can be filled with.

use xui_core::backend::GradientStop;

/// The most stops a [`Gradient`] may have; the drawer keeps them on the stack.
pub const MAX_STOPS: usize = 4;

/// A linear gradient across a shape's bounding box.
///
/// The end points are fractions of the box: `(0, 0)` is its top-left corner and
/// `(1, 1)` its bottom-right, as an SVG `objectBoundingBox` gradient.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Gradient {
    /// Where the gradient starts, as a fraction of the bounding box.
    pub start: (f32, f32),
    /// Where the gradient ends, as a fraction of the bounding box.
    pub end: (f32, f32),
    /// The colours along it: between two and [`MAX_STOPS`].
    pub stops: &'static [GradientStop],
}

impl Gradient {
    /// A gradient from `start` to `end` over `stops`.
    pub const fn new(
        start: (f32, f32),
        end: (f32, f32),
        stops: &'static [GradientStop],
    ) -> Gradient {
        assert!(stops.len() >= 2 && stops.len() <= MAX_STOPS);
        Gradient { start, end, stops }
    }
}
