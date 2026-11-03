#![forbid(unsafe_code)]

//! Styled text for painted nodes: family, size, weight, slant and colour.
//!
//! DirectWrite is the primary path, so a run measures through the same face it
//! is painted with. A thread-local GDI font cache backs the rare case Direct2D
//! is unavailable, so a styled draw does not create a font per frame.

use std::any::Any;
use std::cell::RefCell;
use std::collections::HashMap;

use xui_core::backend::{
    FontSpec as PortableFontSpec, Rgba, TextHit, TextLayout, TextMetrics, TextShaper, TextStyle,
};
use xui_core::geometry::{Point, Rect};

use crate::color::Color;
use crate::d2d::{FontSpec, PointF, RectF, TextSystem};
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
    // Shares the painter's open Direct2D frame; when this is the only call (a
    // standalone draw), the handle owns and ends its own frame.
    let Some(mut d2d) = canvas.d2d() else {
        return false;
    };
    // The layout is cached per (text, width), so repeated cell text is not laid
    // out again every frame.
    font.with_layout(text, max_width, |layout| {
        let (width, height) = layout.size();
        d2d.draw_text(
            layout,
            PointF::new(align_x(style, rect, width), align_y(style, rect, height)),
            style.color,
        );
    })
    .is_ok()
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

/// The DirectWrite request for a portable [`PortableFontSpec`] at `dpi`.
fn portable_spec(spec: &PortableFontSpec, dpi: u32) -> FontSpec {
    let family = spec
        .family
        .clone()
        .unwrap_or_else(|| DEFAULT_FAMILY.to_owned());
    let size_px = spec.size.to_px(dpi).value().max(1) as f32;
    FontSpec::new(family, size_px)
        .weight(spec.weight.value())
        .italic(spec.italic)
}

/// A shaped DirectWrite layout exposed through the portable [`TextLayout`].
pub(crate) struct Win32Layout {
    inner: Option<crate::d2d::Layout>,
}

impl TextLayout for Win32Layout {
    fn width(&self) -> f32 {
        self.inner.as_ref().map_or(0.0, |layout| layout.width())
    }

    fn height(&self) -> f32 {
        self.inner.as_ref().map_or(0.0, |layout| layout.height())
    }

    fn baseline(&self) -> f32 {
        self.inner
            .as_ref()
            .and_then(|layout| layout.lines().first().map(|line| line.baseline))
            .unwrap_or_else(|| self.height() * 0.8)
    }

    fn hit_test_point(&self, x: f32, y: f32) -> TextHit {
        let Some(layout) = &self.inner else {
            return TextHit {
                byte_index: 0,
                inside: false,
            };
        };
        let hit = layout.hit_test_point(x, y);
        TextHit {
            byte_index: hit.index,
            inside: hit.inside,
        }
    }

    fn selection_rects(&self, byte_start: usize, byte_end: usize) -> Vec<Rect> {
        let Some(layout) = &self.inner else {
            return Vec::new();
        };
        layout
            .selection_rects(byte_start, byte_end)
            .into_iter()
            .map(rect_from)
            .collect()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Rounds a DirectWrite rectangle to device pixels.
fn rect_from(rect: RectF) -> Rect {
    Rect::new(
        rect.left.round() as i32,
        rect.top.round() as i32,
        rect.right.round() as i32,
        rect.bottom.round() as i32,
    )
}

/// Shapes `text` with the UI thread's cached DirectWrite system.
pub(crate) fn layout_text(
    text: &str,
    spec: &PortableFontSpec,
    max_width: f32,
    dpi: u32,
) -> Box<dyn TextLayout> {
    let layout = system()
        .and_then(|system| system.font(&portable_spec(spec, dpi)).ok())
        .and_then(|font| font.layout(text, max_width).ok());
    Box::new(Win32Layout { inner: layout })
}

/// A `Send + Sync` DirectWrite shaper the UI thread hands a worker.
pub(crate) struct Win32TextShaper {
    system: Option<TextSystem>,
}

impl Win32TextShaper {
    pub(crate) fn new() -> Win32TextShaper {
        Win32TextShaper {
            system: TextSystem::new().ok(),
        }
    }
}

impl TextShaper for Win32TextShaper {
    fn layout(
        &self,
        text: &str,
        spec: &PortableFontSpec,
        max_width: f32,
        dpi: u32,
    ) -> Box<dyn TextLayout> {
        let layout = self
            .system
            .as_ref()
            .and_then(|system| system.font(&portable_spec(spec, dpi)).ok())
            .and_then(|font| font.layout(text, max_width).ok());
        Box::new(Win32Layout { inner: layout })
    }
}

/// Draws a shaped layout at `origin`, in `color` (alpha ignored by Direct2D).
pub(crate) fn draw_layout(
    canvas: &gdi::Canvas,
    layout: &dyn TextLayout,
    origin: Point,
    color: Rgba,
) -> bool {
    let Some(Win32Layout {
        inner: Some(layout),
    }) = layout.as_any().downcast_ref::<Win32Layout>()
    else {
        return false;
    };
    let Some(mut d2d) = canvas.d2d() else {
        return false;
    };
    d2d.draw_text(
        layout,
        PointF::new(origin.x as f32, origin.y as f32),
        Color::rgb(color.r, color.g, color.b),
    );
    let _ = d2d.end_draw();
    true
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

#[cfg(test)]
mod tests {
    use super::*;
    use xui_core::units::dip;

    #[test]
    fn a_shaped_layout_hit_tests_and_selects() {
        crate::init();
        let shaper = Win32TextShaper::new();
        let spec = PortableFontSpec::new(dip(14.0));
        let layout = shaper.layout("hello world", &spec, f32::INFINITY, 96);

        assert!(layout.width() > 0.0, "DirectWrite measured the run");
        assert!(layout.height() > 0.0);

        let y = layout.height() / 2.0;
        assert_eq!(layout.hit_test_point(0.0, y).byte_index, 0);
        let boxes = layout.selection_rects(6, 11);
        assert!(!boxes.is_empty(), "the selected word has a box");
        assert!(boxes[0].width() > 0);
    }

    #[test]
    fn a_shaped_layout_has_a_baseline_inside_its_first_line() {
        crate::init();
        let shaper = Win32TextShaper::new();
        let layout = shaper.layout(
            "hello",
            &PortableFontSpec::new(dip(14.0)),
            f32::INFINITY,
            96,
        );
        let baseline = layout.baseline();
        assert!(baseline > 0.0 && baseline < layout.height());
    }
}
