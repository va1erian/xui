#![forbid(unsafe_code)]

//! Mapping the portable shape vocabulary onto Win32, and the transform/clip
//! helpers [`Win32Canvas`](super::canvas::Win32Canvas) needs. Keeping them here
//! leaves the canvas module holding the trait implementation alone.

use crate::d2d::{
    Cap as D2dCap, DashStyle, DcCanvas, PointF, Radius, RectF, Rgba as D2dRgba, RoundedRect,
    Stroke as D2dStroke,
};
use crate::gdi::TextFormat;
use xui_core::backend::{
    Cap, Corner, Dash, GradientStop, LinearGradient, RadialGradient, Rgba, Stroke, TextAlign,
    TextStyle, TextVAlign,
};
use xui_core::{Color, Point, Rect};

use super::canvas::Win32Canvas;

/// One entry on the canvas's clip stack: the device-space bounds used to cull
/// a shape and, for a rounded clip, the corners that mask it.
pub(crate) struct Clip {
    pub(crate) bounds: Rect,
    pub(crate) corners: Option<[Corner; 4]>,
}

impl Win32Canvas<'_> {
    pub(crate) fn point(&self, point: Point) -> Point {
        Point::new(
            (self.tx + point.x as f32 * self.scale).round() as i32,
            (self.ty + point.y as f32 * self.scale).round() as i32,
        )
    }

    pub(crate) fn rect(&self, rect: Rect) -> Rect {
        let mapped = Rect::new(
            (self.tx + rect.left as f32 * self.scale).round() as i32,
            (self.ty + rect.top as f32 * self.scale).round() as i32,
            (self.tx + rect.right as f32 * self.scale).round() as i32,
            (self.ty + rect.bottom as f32 * self.scale).round() as i32,
        );
        self.clips
            .last()
            .map_or(mapped, |clip| intersect(mapped, clip.bounds))
    }

    pub(crate) fn d2d(&self) -> Option<DcCanvas> {
        self.canvas.d2d()
    }

    /// Re-applies every open rounded clip to `d2d` for the shape about to be
    /// drawn. The portable canvas binds a fresh Direct2D frame per shape, so a
    /// layer pushed by an earlier call would not survive; re-pushing the clip
    /// keeps it in force for this shape.
    pub(crate) fn push_rounded_clips(&self, d2d: &mut DcCanvas) {
        for clip in &self.clips {
            if let Some(corners) = clip.corners {
                let _ = d2d.push_clip_rounded(rounded(clip.bounds, corners));
            }
        }
    }

    /// GDI cannot draw a gradient; without Direct2D, fill with the gradient's
    /// first stop so the shape is at least opaque.
    pub(crate) fn fill_first_stop(&self, rect: Rect, stop: Option<&GradientStop>) {
        let color = stop.map_or(Color::rgb(0, 0, 0), |stop| {
            Color::rgb(stop.color.r, stop.color.g, stop.color.b)
        });
        self.canvas.fill_rect(rect, color);
    }

    pub(crate) fn text_format(style: &TextStyle) -> TextFormat {
        let format = match style.align {
            TextAlign::Start => TextFormat::left(),
            TextAlign::Center => TextFormat::left().center(),
            TextAlign::End => TextFormat::left().right(),
        };
        let format = match style.valign {
            TextVAlign::Top => format,
            TextVAlign::Middle => format.vcenter(),
        };
        if style.wrap {
            format.word_wrap()
        } else {
            format.single_line()
        }
    }

    /// The device-space bounding box of an ellipse, after the transform.
    pub(crate) fn ellipse_bounds(&self, center: Point, radius_x: f32, radius_y: f32) -> Rect {
        let center = self.point(center);
        let rx = radius_x * self.scale;
        let ry = radius_y * self.scale;
        Rect::new(
            (center.x as f32 - rx).round() as i32,
            (center.y as f32 - ry).round() as i32,
            (center.x as f32 + rx).round() as i32,
            (center.y as f32 + ry).round() as i32,
        )
    }

    /// Whether an ellipse is entirely outside the current clip.
    pub(crate) fn ellipse_clipped_out(&self, bounds: Rect) -> bool {
        self.clips
            .last()
            .is_some_and(|clip| intersect(bounds, clip.bounds).is_empty())
    }
}

/// The intersection of two rectangles.
pub(crate) fn intersect(a: Rect, b: Rect) -> Rect {
    Rect::new(
        a.left.max(b.left),
        a.top.max(b.top),
        a.right.min(b.right),
        a.bottom.min(b.bottom),
    )
}

/// Maps a portable RGBA colour onto the Direct2D one.
pub(crate) fn rgba(color: Rgba) -> D2dRgba {
    D2dRgba::with_alpha(color.r, color.g, color.b, color.a)
}

/// Maps a portable stroke onto the Direct2D one.
pub(crate) fn d2d_stroke(stroke: &Stroke) -> D2dStroke {
    let dash = match stroke.dash {
        Dash::Solid => DashStyle::Solid,
        Dash::Dashed => DashStyle::Dashed,
        Dash::Dotted => DashStyle::Dotted,
    };
    let cap = match stroke.cap {
        Cap::Flat => D2dCap::Flat,
        Cap::Square => D2dCap::Square,
        Cap::Round => D2dCap::Round,
    };
    D2dStroke::solid(stroke.width.max(1.0)).dash(dash).cap(cap)
}

/// Maps device-space bounds and per-corner radii onto a Direct2D rounded
/// rectangle.
pub(crate) fn rounded(rect: Rect, corners: [Corner; 4]) -> RoundedRect {
    RoundedRect::new(
        RectF::from_rect(rect),
        corners.map(|corner| Radius::new(corner.x, corner.y)),
    )
}

/// Maps a portable linear gradient, already in device space, onto Direct2D.
pub(crate) fn linear(gradient: &LinearGradient) -> crate::d2d::LinearGradient {
    crate::d2d::LinearGradient::new(
        point_f(gradient.start),
        point_f(gradient.end),
        gradient.stops.iter().map(gradient_stop).collect(),
    )
}

/// Maps a portable radial gradient, already in device space, onto Direct2D.
pub(crate) fn radial(gradient: &RadialGradient, scale: f32) -> crate::d2d::RadialGradient {
    crate::d2d::RadialGradient::new(
        point_f(gradient.center),
        gradient.radius_x * scale,
        gradient.radius_y * scale,
        gradient.stops.iter().map(gradient_stop).collect(),
    )
}

fn gradient_stop(stop: &GradientStop) -> crate::d2d::GradientStop {
    crate::d2d::GradientStop::new(stop.position, rgba(stop.color))
}

pub(crate) fn point_f(point: Point) -> PointF {
    PointF::new(point.x as f32, point.y as f32)
}
