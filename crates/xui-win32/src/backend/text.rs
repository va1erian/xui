#![forbid(unsafe_code)]

//! Styled text for painted nodes: family, size, weight, slant and colour.
//!
//! DirectWrite is the primary path, so a run measures through the same face it
//! is painted with. A thread-local GDI font cache backs the rare case Direct2D
//! is unavailable, so a styled draw does not create a font per frame.

use std::cell::RefCell;
use std::collections::HashMap;

use xui_core::backend::{TextMetrics, TextStyle};
use xui_core::geometry::Rect;

use crate::d2d::{FontSpec, PointF, TextSystem};
use crate::gdi::{self, TextFormat};
use crate::geometry::Size;

/// The family list a style without an explicit family resolves through.
pub(crate) const DEFAULT_FAMILY: &str = "system-ui, Segoe UI, Arial, sans-serif";

thread_local! {
    static SYSTEM: RefCell<Option<TextSystem>> = const { RefCell::new(None) };
    static GDI_FONTS: RefCell<HashMap<GdiKey, gdi::Font>> = RefCell::new(HashMap::new());
}

/// A GDI font cache key: the resolved family, device-pixel height, weight and
/// slant.
#[derive(Clone, PartialEq, Eq, Hash)]
struct GdiKey {
    family: String,
    height: i32,
    weight: u16,
    italic: bool,
}

/// The DirectWrite text system, created once per UI thread.
fn system() -> Option<TextSystem> {
    SYSTEM.with(|cell| {
        let mut slot = cell.borrow_mut();
        if slot.is_none() {
            *slot = TextSystem::new().ok();
        }
        slot.clone()
    })
}

/// The DirectWrite request for `style` at `dpi`. The Direct2D DC target is one
/// device pixel per unit, so the face is sized in device pixels.
fn font_spec(style: &TextStyle, dpi: u32) -> FontSpec {
    let family = style
        .family
        .clone()
        .unwrap_or_else(|| DEFAULT_FAMILY.to_owned());
    let size_px = style.size.to_px(dpi).value().max(1) as f32;
    FontSpec::new(family, size_px)
        .weight(style.weight.value())
        .italic(style.italic)
}

/// The resolved family and device-pixel height for the GDI fallback.
fn gdi_spec(style: &TextStyle, dpi: u32) -> (String, i32) {
    let family = style
        .family
        .clone()
        .unwrap_or_else(crate::gdi::system_ui_family);
    (family, style.size.to_px(dpi).value().max(1))
}

/// The horizontal origin of a `width`-wide layout inside `rect`.
fn align_x(style: &TextStyle, rect: Rect, width: f32) -> f32 {
    use xui_core::backend::TextAlign;
    match style.align {
        TextAlign::Start => rect.left as f32,
        TextAlign::Center => rect.left as f32 + (rect.width() as f32 - width) / 2.0,
        TextAlign::End => rect.right as f32 - width,
    }
}

/// The vertical origin of a `height`-tall layout inside `rect`.
fn align_y(style: &TextStyle, rect: Rect, height: f32) -> f32 {
    use xui_core::backend::TextVAlign;
    match style.valign {
        TextVAlign::Top => rect.top as f32,
        TextVAlign::Middle => rect.top as f32 + (rect.height() as f32 - height) / 2.0,
    }
}

/// Measures `text` with DirectWrite in device pixels; `None` when DirectWrite
/// is unavailable.
pub(crate) fn measure_d2d(text: &str, style: &TextStyle, dpi: u32) -> Option<TextMetrics> {
    let font = system()?.font(&font_spec(style, dpi)).ok()?;
    let metrics = font.metrics();
    let height = metrics.line_height().round().max(1.0) as i32;
    Some(TextMetrics {
        width: font.width(text).round() as i32,
        height,
        ascent: metrics.ascent.round() as i32,
        descent: metrics.descent.round() as i32,
    })
}

/// Draws `text` into `rect` (device pixels) with DirectWrite, aligned per
/// `style`. Returns `false` when DirectWrite is unavailable, so the caller can
/// fall back to GDI.
pub(crate) fn draw_d2d(
    canvas: &gdi::Canvas,
    text: &str,
    rect: Rect,
    style: &TextStyle,
    dpi: u32,
) -> bool {
    let Some(font) = system().and_then(|system| system.font(&font_spec(style, dpi)).ok()) else {
        return false;
    };
    let max_width = if style.wrap {
        rect.width().max(1) as f32
    } else {
        f32::INFINITY
    };
    let Ok(layout) = font.layout(text, max_width) else {
        return false;
    };
    let (width, height) = layout.size();
    let Some(mut d2d) = canvas.d2d() else {
        return false;
    };
    d2d.draw_text(
        &layout,
        PointF::new(align_x(style, rect, width), align_y(style, rect, height)),
        style.color,
    );
    let _ = d2d.end_draw();
    true
}

/// The GDI font cache key for `style` at `dpi`.
fn gdi_key(style: &TextStyle, dpi: u32) -> GdiKey {
    let (family, height) = gdi_spec(style, dpi);
    GdiKey {
        family,
        height,
        weight: style.weight.value(),
        italic: style.italic,
    }
}

/// Draws `text` into `rect` (device pixels), aligned per `style`, through
/// DirectWrite and falling back to a cached GDI font.
pub(crate) fn draw(
    canvas: &gdi::Canvas,
    text: &str,
    rect: Rect,
    style: &TextStyle,
    dpi: u32,
    format: TextFormat,
) {
    if rect.is_empty() {
        return;
    }
    if !draw_d2d(canvas, text, rect, style, dpi) {
        draw_gdi(canvas, text, rect, style, dpi, format);
    }
}

/// Measures `text` in a cached GDI font when DirectWrite is unavailable.
pub(crate) fn measure_gdi(text: &str, style: &TextStyle, dpi: u32) -> TextMetrics {
    GDI_FONTS.with(|cell| {
        let mut fonts = cell.borrow_mut();
        let key = gdi_key(style, dpi);
        if !fonts.contains_key(&key)
            && let Ok(font) = gdi::Font::styled(&key.family, key.height, key.weight, key.italic)
        {
            fonts.insert(key.clone(), font);
        }
        let Some(font) = fonts.get(&key) else {
            return TextMetrics::default();
        };
        let Size { width, height } = crate::sys::gdi::measure_text(font.raw(), text);
        TextMetrics {
            width,
            height,
            ascent: height * 3 / 4,
            descent: height / 4,
        }
    })
}

/// Draws `text` into `rect` with a cached GDI font when DirectWrite is
/// unavailable, aligned with `format`.
pub(crate) fn draw_gdi(
    canvas: &gdi::Canvas,
    text: &str,
    rect: Rect,
    style: &TextStyle,
    dpi: u32,
    format: TextFormat,
) {
    GDI_FONTS.with(|cell| {
        let mut fonts = cell.borrow_mut();
        let key = gdi_key(style, dpi);
        if !fonts.contains_key(&key)
            && let Ok(font) = gdi::Font::styled(&key.family, key.height, key.weight, key.italic)
        {
            fonts.insert(key.clone(), font);
        }
        if let Some(font) = fonts.get(&key) {
            canvas.with_font(font, |canvas| {
                canvas.draw_text(rect, text, style.color, format);
            });
        }
    });
}
