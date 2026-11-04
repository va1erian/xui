//! Direct2D drawing and DirectWrite text behind a safe, xui-flavoured API.
//!
//! Everything here is in device pixels. A [`Target`] is one window's render
//! target; [`Geometry`] is a reusable path; [`DwShaper`] and [`DwLayout`] are
//! the thread-safe text shaping handle and its layouts.

mod bitmap;
mod factory;
mod geometry;
mod layout;
mod paint;
mod shapes;
mod target;
mod text;

pub(crate) use geometry::Geometry;
pub(crate) use layout::{DwLayout, DwShaper, NoLayout};
pub(crate) use target::{Frame, Target};
pub(crate) use text::{TextParams, measure};

use xui_core::Rect;

/// A rectangle in floating-point device pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct RectF {
    pub(crate) left: f32,
    pub(crate) top: f32,
    pub(crate) right: f32,
    pub(crate) bottom: f32,
}

impl RectF {
    pub(crate) fn new(left: f32, top: f32, right: f32, bottom: f32) -> RectF {
        RectF {
            left,
            top,
            right,
            bottom,
        }
    }

    pub(crate) fn width(self) -> f32 {
        self.right - self.left
    }

    pub(crate) fn height(self) -> f32 {
        self.bottom - self.top
    }
}

impl From<Rect> for RectF {
    fn from(rect: Rect) -> RectF {
        RectF::new(
            rect.left as f32,
            rect.top as f32,
            rect.right as f32,
            rect.bottom as f32,
        )
    }
}
