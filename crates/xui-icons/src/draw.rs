#![forbid(unsafe_code)]

//! Drawing an icon on a [`Canvas`].

use xui_core::backend::{Canvas, Cap, Join, PathPlacement, Rgba, Stroke};
use xui_core::geometry::Rect;

use crate::icon::Icon;
use crate::shape::Paint;
use crate::tone::Palette;

/// The design grid every icon is authored on, in path units.
pub const GRID: f32 = 32.0;

/// The thinnest stroke drawn, in device pixels, so the outline of a small icon
/// stays visible instead of fading to a faint line.
const MIN_STROKE_PX: f32 = 0.75;

/// Draws `icon` centred in `rect`, scaled so the grid spans the rectangle's
/// smaller side, in the colours of `palette`.
///
/// It goes through [`Canvas::fill_path`] and [`Canvas::stroke_path`] only, so
/// every backend draws it: `xui-canvas` rasterises it with `tiny-skia`. Nothing
/// is drawn for an empty rectangle. Icons are multi-coloured, so they take a
/// [`Palette`] rather than a single colour; the palette recolours the
/// [`Tone`](crate::Tone) roles.
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
    for shape in icon.shapes() {
        if let Some(fill) = shape.fill {
            canvas.fill_path(shape.path, at, fade(solid(fill, palette), shape.opacity));
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

/// The colour of `paint`.
fn solid(paint: Paint, palette: &Palette) -> Rgba {
    match paint {
        Paint::Tone(tone) => palette.get(tone),
        Paint::Color(color) => color,
    }
}

/// `color` with its alpha scaled by `opacity / 255`.
fn fade(color: Rgba, opacity: u8) -> Rgba {
    let alpha = u16::from(color.a) * u16::from(opacity) / 255;
    Rgba::with_alpha(color.r, color.g, color.b, alpha as u8)
}
