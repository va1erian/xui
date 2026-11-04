//! Solid shapes, strokes and clips.

use windows::Win32::Graphics::Direct2D::Common::D2D_RECT_F;
use windows::Win32::Graphics::Direct2D::{
    D2D1_ANTIALIAS_MODE_PER_PRIMITIVE, D2D1_ELLIPSE, D2D1_ROUNDED_RECT, ID2D1StrokeStyle,
};
use windows_numerics::Vector2;
use xui_core::backend::{Rgba, Stroke};

use super::geometry::Geometry;
use super::target::Target;
use super::{RectF, factory};

pub(super) fn rect_f(rect: RectF) -> D2D_RECT_F {
    D2D_RECT_F {
        left: rect.left,
        top: rect.top,
        right: rect.right,
        bottom: rect.bottom,
    }
}

fn point(x: f32, y: f32) -> Vector2 {
    Vector2 { X: x, Y: y }
}

impl Target {
    pub(crate) fn fill_rect(&self, rect: RectF, color: Rgba) {
        let brush = self.solid(color);
        // SAFETY: inside a frame; the rect and brush outlive the call.
        unsafe { self.rt.FillRectangle(&rect_f(rect), brush) };
    }

    pub(crate) fn stroke_rect(&self, rect: RectF, color: Rgba, width: f32) {
        let brush = self.solid(color);
        // SAFETY: inside a frame.
        unsafe { self.rt.DrawRectangle(&rect_f(rect), brush, width, None) };
    }

    pub(crate) fn fill_round_rect(&self, rect: RectF, radius: f32, color: Rgba) {
        let brush = self.solid(color);
        // SAFETY: inside a frame.
        unsafe { self.rt.FillRoundedRectangle(&rounded(rect, radius), brush) };
    }

    pub(crate) fn stroke_round_rect(&self, rect: RectF, radius: f32, color: Rgba, width: f32) {
        let brush = self.solid(color);
        // SAFETY: inside a frame.
        unsafe {
            self.rt
                .DrawRoundedRectangle(&rounded(rect, radius), brush, width, None)
        };
    }

    pub(crate) fn fill_ellipse(&self, center: (f32, f32), radii: (f32, f32), color: Rgba) {
        let brush = self.solid(color);
        // SAFETY: inside a frame.
        unsafe { self.rt.FillEllipse(&ellipse(center, radii), brush) };
    }

    pub(crate) fn stroke_ellipse(
        &self,
        center: (f32, f32),
        radii: (f32, f32),
        color: Rgba,
        stroke: &Stroke,
    ) {
        let brush = self.solid(color);
        let style = style_of(stroke);
        // SAFETY: inside a frame.
        unsafe {
            self.rt
                .DrawEllipse(&ellipse(center, radii), brush, stroke.width, style.as_ref())
        };
    }

    pub(crate) fn line(&self, from: (f32, f32), to: (f32, f32), color: Rgba, stroke: &Stroke) {
        let brush = self.solid(color);
        let style = style_of(stroke);
        // SAFETY: inside a frame.
        unsafe {
            self.rt.DrawLine(
                point(from.0, from.1),
                point(to.0, to.1),
                brush,
                stroke.width,
                style.as_ref(),
            )
        };
    }

    pub(crate) fn fill_geometry(&self, geometry: &Geometry, color: Rgba) {
        let brush = self.solid(color);
        // SAFETY: inside a frame.
        unsafe { self.rt.FillGeometry(geometry.raw(), brush, None) };
    }

    pub(crate) fn stroke_geometry(&self, geometry: &Geometry, color: Rgba, stroke: &Stroke) {
        let brush = self.solid(color);
        let style = style_of(stroke);
        // SAFETY: inside a frame.
        unsafe {
            self.rt
                .DrawGeometry(geometry.raw(), brush, stroke.width, style.as_ref())
        };
    }

    /// Clips to `rect` until [`Target::pop_clip`].
    pub(crate) fn push_clip(&self, rect: RectF) {
        // SAFETY: inside a frame; popped by the matching `pop_clip`.
        unsafe {
            self.rt
                .PushAxisAlignedClip(&rect_f(rect), D2D1_ANTIALIAS_MODE_PER_PRIMITIVE)
        };
    }

    pub(crate) fn pop_clip(&self) {
        // SAFETY: pops the clip pushed by `push_clip`.
        unsafe { self.rt.PopAxisAlignedClip() };
    }
}

fn rounded(rect: RectF, radius: f32) -> D2D1_ROUNDED_RECT {
    let radius = radius
        .min(rect.width() / 2.0)
        .min(rect.height() / 2.0)
        .max(0.0);
    D2D1_ROUNDED_RECT {
        rect: rect_f(rect),
        radiusX: radius,
        radiusY: radius,
    }
}

fn ellipse(center: (f32, f32), radii: (f32, f32)) -> D2D1_ELLIPSE {
    D2D1_ELLIPSE {
        point: point(center.0, center.1),
        radiusX: radii.0,
        radiusY: radii.1,
    }
}

/// The cached stroke style for `stroke`, or `None` for the plain solid one.
fn style_of(stroke: &Stroke) -> Option<ID2D1StrokeStyle> {
    factory::with(|f| f.stroke_style(stroke)).flatten()
}
