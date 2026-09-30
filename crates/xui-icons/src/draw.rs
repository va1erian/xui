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
/// is degenerate along the gradient.
fn gradient_span(path: &[PathSeg], gradient: &Gradient) -> Option<((f32, f32), (f32, f32))> {
    let (mut left, mut top, mut right, mut bottom) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    let mut include = |x: f32, y: f32| {
        left = left.min(x);
        right = right.max(x);
        top = top.min(y);
        bottom = bottom.max(y);
    };
    for seg in path {
        match *seg {
            PathSeg::MoveTo(x, y) | PathSeg::LineTo(x, y) => include(x, y),
            PathSeg::CubicTo(x1, y1, x2, y2, x, y) => {
                include(x1, y1);
                include(x2, y2);
                include(x, y);
            }
            PathSeg::Close => {}
        }
    }
    let (width, height) = (right - left, bottom - top);
    let at = |(fx, fy): (f32, f32)| (left + fx * width, top + fy * height);
    let (start, end) = (at(gradient.start), at(gradient.end));
    (width >= 0.0 && height >= 0.0 && start != end).then_some((start, end))
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
