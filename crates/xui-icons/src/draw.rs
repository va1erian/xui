#![forbid(unsafe_code)]

//! Drawing an icon on a [`Canvas`].

use xui_core::backend::{
    Canvas, Cap, GradientStop, Join, PathGradient, PathPlacement, PathSeg, Rgba, Stroke,
};
use xui_core::geometry::Rect;

use crate::gradient::{Gradient, MAX_STOPS};
use crate::icon::Icon;
use crate::shape::{Paint, Shape};
use crate::tone::Palette;
use crate::{STYLE, Style};

/// The design grid every icon is authored on, in path units.
pub const GRID: f32 = 32.0;

/// The thinnest stroke drawn, in device pixels, so the outline of a small icon
/// stays visible instead of fading to a faint line.
const MIN_STROKE_PX: f32 = 0.75;

/// The soft shadow of the [`Style::Aero`] artwork, drawn one grid unit down.
const SHADOW: Rgba = Rgba::with_alpha(0x00, 0x14, 0x2E, 0x59);

/// Draws `icon` centred in `rect`, scaled so the grid spans the rectangle's
/// smaller side, in the colours of `palette`.
///
/// It goes through [`Canvas::fill_path`], [`Canvas::fill_path_linear`] and
/// [`Canvas::stroke_path`] only, so every backend draws it: `xui-canvas`
/// rasterises it with `tiny-skia`. A backend without gradient paths fills
/// gradient shapes with their middle colour. Nothing is drawn for an empty
/// rectangle. Icons are multi-coloured, so they take a [`Palette`] rather than
/// a single colour; the palette recolours the [`Tone`](crate::Tone) roles of
/// [`Style::GlobalVillage`] and leaves the fixed colours of [`Style::Aero`].
///
/// Gradient fills build a small colour list on the stack, but the backend may
/// allocate one per gradient shape while rasterising; that is unavoidable with
/// `tiny-skia`.
///
/// ```no_run
/// use xui_icons::{Icon, Palette, draw};
/// # fn paint(canvas: &mut dyn xui_core::backend::Canvas, rect: xui_core::Rect) {
/// draw(canvas, Icon::Folder, rect, &Palette::GLOBAL_VILLAGE);
/// # }
/// ```
pub fn draw(canvas: &mut dyn Canvas, icon: Icon, rect: Rect, palette: &Palette) {
    let size = rect.width().min(rect.height());
    if size <= 0 {
        return;
    }
    let scale = size as f32 / GRID;
    let at = PathPlacement::new(
        scale,
        (rect.left + rect.right - size) as f32 / 2.0,
        (rect.top + rect.bottom - size) as f32 / 2.0,
    );
    if STYLE == Style::Aero {
        let below = PathPlacement::new(scale, at.x, at.y + scale);
        for shape in icon.shapes().iter().filter(|shape| casts_shadow(shape)) {
            canvas.fill_path(shape.path, below, SHADOW);
        }
    }
    for shape in icon.shapes() {
        if let Some(fill) = shape.fill {
            fill_shape(canvas, shape, fill, at, palette);
        }
        if let Some(line) = shape.line {
            let stroke = Stroke::new((line.width * scale).max(MIN_STROKE_PX))
                .cap(Cap::Round)
                .join(Join::Round);
            let color = fade(solid(line.paint, palette), shape.opacity);
            canvas.stroke_path(shape.path, at, color, &stroke);
        }
    }
}

/// Whether `shape` is a solid filled body that should throw a shadow (glints
/// and other translucent overlays do not).
fn casts_shadow(shape: &Shape) -> bool {
    shape.opacity == 255
        && match shape.fill {
            Some(Paint::Tone(_)) => true,
            Some(Paint::Color(color)) => color.a == 255,
            Some(Paint::Gradient(gradient)) => {
                gradient.stops.iter().all(|stop| stop.color.a == 255)
            }
            None => false,
        }
}

fn fill_shape(
    canvas: &mut dyn Canvas,
    shape: &Shape,
    fill: Paint,
    at: PathPlacement,
    palette: &Palette,
) {
    if let Paint::Gradient(gradient) = fill
        && let Some((start, end)) = gradient_span(shape.path, gradient)
    {
        let mut stops = [GradientStop::new(0.0, Rgba::TRANSPARENT); MAX_STOPS];
        for (slot, stop) in stops.iter_mut().zip(gradient.stops) {
            *slot = GradientStop::new(stop.position, fade(stop.color, shape.opacity));
        }
        let stops = &stops[..gradient.stops.len().min(MAX_STOPS)];
        canvas.fill_path_linear(shape.path, at, &PathGradient::new(start, end, stops));
    } else {
        canvas.fill_path(shape.path, at, fade(solid(fill, palette), shape.opacity));
    }
}

/// The end points of `gradient` in path space, or `None` when the bounding box
/// is degenerate along the gradient. The box is the path's true extent: a
/// curve's control points can lie well outside it, so its extrema are used.
fn gradient_span(path: &[PathSeg], gradient: &Gradient) -> Option<((f32, f32), (f32, f32))> {
    let (mut left, mut top, mut right, mut bottom) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    let mut include = |(x, y): (f32, f32)| {
        left = left.min(x);
        right = right.max(x);
        top = top.min(y);
        bottom = bottom.max(y);
    };
    let (mut current, mut start) = ((0.0, 0.0), (0.0, 0.0));
    for seg in path {
        match *seg {
            PathSeg::MoveTo(x, y) => {
                current = (x, y);
                start = current;
                include(current);
            }
            PathSeg::LineTo(x, y) => {
                current = (x, y);
                include(current);
            }
            PathSeg::CubicTo(x1, y1, x2, y2, x, y) => {
                let end = (x, y);
                include(end);
                for t in cubic_extrema(current.0, x1, x2, x)
                    .into_iter()
                    .chain(cubic_extrema(current.1, y1, y2, y))
                    .flatten()
                {
                    include(cubic_at(current, (x1, y1), (x2, y2), end, t));
                }
                current = end;
            }
            PathSeg::Close => current = start,
        }
    }
    let (width, height) = (right - left, bottom - top);
    let at = |(fx, fy): (f32, f32)| (left + fx * width, top + fy * height);
    let (start, end) = (at(gradient.start), at(gradient.end));
    (width >= 0.0 && height >= 0.0 && start != end).then_some((start, end))
}

/// The parameters in `(0, 1)` where one coordinate of a cubic Bézier (`p0`
/// to `p3` with controls `p1`, `p2`) has a turning point.
fn cubic_extrema(p0: f32, p1: f32, p2: f32, p3: f32) -> [Option<f32>; 2] {
    // The derivative is 3 (a t² + b t + c).
    let (d0, d1, d2) = (p1 - p0, p2 - p1, p3 - p2);
    let (a, b, c) = (d0 - 2.0 * d1 + d2, 2.0 * (d1 - d0), d0);
    let inside = |t: f32| (t > 0.0 && t < 1.0).then_some(t);
    if a.abs() < 1e-6 {
        return [(b.abs() > 1e-6).then(|| -c / b).and_then(inside), None];
    }
    let discriminant = b * b - 4.0 * a * c;
    if discriminant < 0.0 {
        return [None, None];
    }
    let root = discriminant.sqrt();
    [
        inside((-b + root) / (2.0 * a)),
        inside((-b - root) / (2.0 * a)),
    ]
}

/// The point at `t` on the cubic Bézier `p0`, `p1`, `p2`, `p3`.
fn cubic_at(p0: (f32, f32), p1: (f32, f32), p2: (f32, f32), p3: (f32, f32), t: f32) -> (f32, f32) {
    let u = 1.0 - t;
    let (w0, w1, w2, w3) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
    (
        w0 * p0.0 + w1 * p1.0 + w2 * p2.0 + w3 * p3.0,
        w0 * p0.1 + w1 * p1.1 + w2 * p2.1 + w3 * p3.1,
    )
}

/// The flat colour of `paint`: a gradient contributes its middle stop.
fn solid(paint: Paint, palette: &Palette) -> Rgba {
    match paint {
        Paint::Tone(tone) => palette.get(tone),
        Paint::Color(color) => color,
        Paint::Gradient(gradient) => gradient.stops[gradient.stops.len() / 2].color,
    }
}

/// `color` with its alpha scaled by `opacity / 255`.
fn fade(color: Rgba, opacity: u8) -> Rgba {
    let alpha = u16::from(color.a) * u16::from(opacity) / 255;
    Rgba::with_alpha(color.r, color.g, color.b, alpha as u8)
}

#[cfg(test)]
mod tests {
    use super::*;

    const DOWN: Gradient = Gradient::new(
        (0.0, 0.0),
        (0.0, 1.0),
        &[
            GradientStop::new(0.0, Rgba::TRANSPARENT),
            GradientStop::new(1.0, Rgba::TRANSPARENT),
        ],
    );

    #[test]
    fn a_gradient_follows_the_curve_not_its_control_points() {
        // A half-disc bowl: its control points reach y = 26 but the curve
        // bottoms out at 0.75 * 26 = 19.5.
        let bowl = [
            PathSeg::MoveTo(0.0, 0.0),
            PathSeg::CubicTo(0.0, 26.0, 20.0, 26.0, 20.0, 0.0),
            PathSeg::Close,
        ];
        let (start, end) = gradient_span(&bowl, &DOWN).unwrap();
        assert_eq!(start, (0.0, 0.0));
        assert!((end.1 - 19.5).abs() < 1e-3, "ended at {}", end.1);
    }

    #[test]
    fn a_degenerate_box_has_no_span() {
        let dot = [PathSeg::MoveTo(3.0, 3.0), PathSeg::LineTo(3.0, 3.0)];
        assert_eq!(gradient_span(&dot, &DOWN), None);
    }
}
