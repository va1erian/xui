//! DirectWrite text: formats, one-shot drawing and measuring.

use std::cell::RefCell;
use std::collections::HashMap;

use windows::Win32::Graphics::Direct2D::D2D1_DRAW_TEXT_OPTIONS_NONE;
use windows::Win32::Graphics::DirectWrite::{
    DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE, DWRITE_FONT_STYLE_ITALIC,
    DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_WEIGHT, DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
    DWRITE_PARAGRAPH_ALIGNMENT_NEAR, DWRITE_TEXT_ALIGNMENT_CENTER, DWRITE_TEXT_ALIGNMENT_LEADING,
    DWRITE_TEXT_ALIGNMENT_TRAILING, DWRITE_TEXT_METRICS, DWRITE_WORD_WRAPPING_NO_WRAP,
    DWRITE_WORD_WRAPPING_WRAP, IDWriteFactory, IDWriteTextFormat, IDWriteTextLayout,
};
use windows::core::HSTRING;
use windows_numerics::Vector2;
use xui_core::backend::{Rgba, TextAlign, TextVAlign};

use super::layout_cache::shape_cached;
use super::target::Target;
use super::{RectF, factory};

/// The family used when a style names none.
const DEFAULT_FAMILY: &str = "Segoe UI";

/// The text to draw or measure, in device pixels.
#[derive(Clone, Debug)]
pub(crate) struct TextParams<'a> {
    pub(crate) family: Option<&'a str>,
    pub(crate) size_px: f32,
    pub(crate) weight: u16,
    pub(crate) italic: bool,
    pub(crate) align: TextAlign,
    pub(crate) valign: TextVAlign,
    pub(crate) wrap: bool,
}

#[derive(Clone, PartialEq, Eq, Hash)]
struct FormatKey {
    family: String,
    size_bits: u32,
    weight: u16,
    italic: bool,
}

thread_local! {
    static FORMATS: RefCell<HashMap<FormatKey, IDWriteTextFormat>> = RefCell::new(HashMap::new());
}

fn family_name(family: Option<&str>) -> &str {
    match family {
        None => DEFAULT_FAMILY,
        Some(name) if name.eq_ignore_ascii_case("monospace") => "Consolas",
        Some(name) => name,
    }
}

/// The text format for a font, built once and shared.
pub(super) fn format(
    factory: &IDWriteFactory,
    family: Option<&str>,
    size_px: f32,
    weight: u16,
    italic: bool,
) -> Option<IDWriteTextFormat> {
    let family = family_name(family);
    let key = FormatKey {
        family: family.to_owned(),
        size_bits: size_px.to_bits(),
        weight,
        italic,
    };
    if let Some(found) = FORMATS.with(|formats| formats.borrow().get(&key).cloned()) {
        return Some(found);
    }
    let style: DWRITE_FONT_STYLE = if italic {
        DWRITE_FONT_STYLE_ITALIC
    } else {
        DWRITE_FONT_STYLE_NORMAL
    };
    // SAFETY: the strings outlive the call; DirectWrite copies them.
    let created = unsafe {
        factory.CreateTextFormat(
            &HSTRING::from(family),
            None,
            DWRITE_FONT_WEIGHT(i32::from(weight)),
            style,
            DWRITE_FONT_STRETCH_NORMAL,
            size_px.max(1.0),
            &HSTRING::from("en-us"),
        )
    }
    .ok()?;
    FORMATS.with(|formats| formats.borrow_mut().insert(key, created.clone()));
    Some(created)
}

/// Shapes `text` for drawing or measuring with `params`, in a box of `width` x
/// `height` pixels.
pub(super) fn shape(
    factory: &IDWriteFactory,
    text: &str,
    params: &TextParams<'_>,
    width: f32,
    height: f32,
) -> Option<IDWriteTextLayout> {
    let format = format(
        factory,
        params.family,
        params.size_px,
        params.weight,
        params.italic,
    )?;
    let units: Vec<u16> = text.encode_utf16().collect();
    // SAFETY: `units` outlives the call; the setters are plain state changes
    // on the layout just created.
    unsafe {
        let layout = factory
            .CreateTextLayout(&units, &format, width.max(0.0), height.max(0.0))
            .ok()?;
        let _ = layout.SetTextAlignment(match params.align {
            TextAlign::Start => DWRITE_TEXT_ALIGNMENT_LEADING,
            TextAlign::Center => DWRITE_TEXT_ALIGNMENT_CENTER,
            TextAlign::End => DWRITE_TEXT_ALIGNMENT_TRAILING,
        });
        let _ = layout.SetParagraphAlignment(match params.valign {
            TextVAlign::Top => DWRITE_PARAGRAPH_ALIGNMENT_NEAR,
            TextVAlign::Middle => DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
        });
        let _ = layout.SetWordWrapping(if params.wrap {
            DWRITE_WORD_WRAPPING_WRAP
        } else {
            DWRITE_WORD_WRAPPING_NO_WRAP
        });
        Some(layout)
    }
}

/// The advance width, height and first baseline of `text`, in device pixels.
pub(crate) fn measure(text: &str, params: &TextParams<'_>) -> (f32, f32, f32) {
    factory::with(|f| {
        let layout = shape_cached(&f.dwrite, text, params, f32::MAX, f32::MAX)?;
        let mut metrics = DWRITE_TEXT_METRICS::default();
        // SAFETY: `metrics` is a valid out-pointer.
        unsafe { layout.GetMetrics(&mut metrics) }.ok()?;
        Some((
            metrics.widthIncludingTrailingWhitespace,
            metrics.height,
            super::layout::first_baseline(&layout),
        ))
    })
    .flatten()
    .unwrap_or((0.0, 0.0, 0.0))
}

impl Target {
    /// Draws `text` inside `rect`, clipped to it.
    pub(crate) fn draw_text(&self, text: &str, rect: RectF, params: &TextParams<'_>, color: Rgba) {
        if rect.width() <= 0.0 || rect.height() <= 0.0 || text.is_empty() {
            return;
        }
        let Some(layout) =
            factory::with(|f| shape_cached(&f.dwrite, text, params, rect.width(), rect.height()))
                .flatten()
        else {
            return;
        };
        let brush = self.solid(color);
        self.push_clip(rect);
        // SAFETY: inside a frame; the layout and brush outlive the call.
        unsafe {
            self.rt.DrawTextLayout(
                Vector2 {
                    X: rect.left,
                    Y: rect.top,
                },
                &layout,
                brush,
                D2D1_DRAW_TEXT_OPTIONS_NONE,
            )
        };
        self.pop_clip();
    }
}
