//! Shaped text for the portable `TextShaper` / `TextLayout` contract.

use std::any::Any;
use std::sync::Mutex;

use windows::Win32::Graphics::Direct2D::D2D1_DRAW_TEXT_OPTIONS_NONE;
use windows::Win32::Graphics::DirectWrite::{
    DWRITE_HIT_TEST_METRICS, DWRITE_LINE_METRICS, DWRITE_TEXT_METRICS, IDWriteFactory,
    IDWriteTextLayout,
};
use windows::core::BOOL;
use windows_numerics::Vector2;
use xui_core::Rect;
use xui_core::backend::{FontSpec, Rgba, TextAlign, TextHit, TextLayout, TextShaper, TextVAlign};

use super::factory;
use super::target::Target;
use super::text::{TextParams, shape};

/// A DirectWrite object moved across threads.
///
/// SAFETY of the impls: the shared DirectWrite factory is documented as
/// thread-safe, and every layout is only ever used behind a `Mutex`, so no two
/// threads touch one object at once.
struct Guarded<T>(T);

// SAFETY: see `Guarded`.
unsafe impl<T> Send for Guarded<T> {}
// SAFETY: see `Guarded`.
unsafe impl<T> Sync for Guarded<T> {}

/// The distance from the top of `layout` to its first line's baseline.
pub(super) fn first_baseline(layout: &IDWriteTextLayout) -> f32 {
    let mut count = 0;
    // SAFETY: a first call with no buffer reports how many lines there are.
    let _ = unsafe { layout.GetLineMetrics(None, &mut count) };
    let mut lines = vec![DWRITE_LINE_METRICS::default(); count.max(1) as usize];
    // SAFETY: the buffer holds `count` entries and `count` is a valid out-pointer.
    let _ = unsafe { layout.GetLineMetrics(Some(&mut lines), &mut count) };
    lines[0].baseline
}

/// A `Send + Sync` handle that shapes text with DirectWrite.
#[derive(Clone)]
pub(crate) struct DwShaper(std::sync::Arc<Guarded<IDWriteFactory>>);

impl DwShaper {
    /// A shaper over the thread's DirectWrite factory, or `None` when
    /// DirectWrite is unavailable.
    pub(crate) fn new() -> Option<DwShaper> {
        factory::with(|f| DwShaper(std::sync::Arc::new(Guarded(f.dwrite.clone()))))
    }
}

impl TextShaper for DwShaper {
    fn layout(&self, text: &str, spec: &FontSpec, max_width: f32, dpi: u32) -> Box<dyn TextLayout> {
        let params = TextParams {
            family: spec.family.as_deref(),
            size_px: spec.size.to_px(dpi).value() as f32,
            weight: spec.weight.value(),
            italic: spec.italic,
            align: TextAlign::Start,
            valign: TextVAlign::Top,
            wrap: max_width.is_finite(),
        };
        let width = if max_width.is_finite() {
            max_width
        } else {
            f32::MAX
        };
        match shape(&self.0.0, text, &params, width, f32::MAX) {
            Some(layout) => Box::new(DwLayout::new(layout, text)),
            None => Box::new(NoLayout),
        }
    }
}

/// A shaped layout, with the maps between UTF-8 byte offsets and the UTF-16
/// offsets DirectWrite uses.
pub(crate) struct DwLayout {
    inner: Mutex<Guarded<IDWriteTextLayout>>,
    text: String,
    /// The byte offset of each UTF-16 unit, plus the text's length.
    byte_of_unit: Vec<usize>,
    width: f32,
    height: f32,
    baseline: f32,
}

impl DwLayout {
    fn new(layout: IDWriteTextLayout, text: &str) -> DwLayout {
        let mut metrics = DWRITE_TEXT_METRICS::default();
        // SAFETY: `metrics` is a valid out-pointer.
        let _ = unsafe { layout.GetMetrics(&mut metrics) };
        let mut byte_of_unit = Vec::with_capacity(text.len() + 1);
        for (at, character) in text.char_indices() {
            byte_of_unit.extend(std::iter::repeat_n(at, character.len_utf16()));
        }
        byte_of_unit.push(text.len());
        DwLayout {
            baseline: first_baseline(&layout),
            inner: Mutex::new(Guarded(layout)),
            text: text.to_owned(),
            byte_of_unit,
            width: metrics.widthIncludingTrailingWhitespace,
            height: metrics.height,
        }
    }

    fn unit_of_byte(&self, byte: usize) -> u32 {
        let mut byte = byte.min(self.text.len());
        while !self.text.is_char_boundary(byte) {
            byte -= 1;
        }
        self.text[..byte].encode_utf16().count() as u32
    }

    fn byte_of_unit(&self, unit: u32) -> usize {
        let last = self.byte_of_unit.len() - 1;
        self.byte_of_unit[(unit as usize).min(last)]
    }
}

impl TextLayout for DwLayout {
    fn width(&self) -> f32 {
        self.width
    }

    fn height(&self) -> f32 {
        self.height
    }

    fn baseline(&self) -> f32 {
        self.baseline
    }

    fn hit_test_point(&self, x: f32, y: f32) -> TextHit {
        let guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let (mut trailing, mut inside) = (BOOL(0), BOOL(0));
        let mut metrics = DWRITE_HIT_TEST_METRICS::default();
        // SAFETY: all three out-pointers are valid for the call.
        let hit = unsafe {
            guard
                .0
                .HitTestPoint(x, y, &mut trailing, &mut inside, &mut metrics)
        };
        if hit.is_err() {
            return TextHit {
                byte_index: 0,
                inside: false,
            };
        }
        let unit = metrics.textPosition
            + if trailing.as_bool() {
                metrics.length
            } else {
                0
            };
        TextHit {
            byte_index: self.byte_of_unit(unit),
            inside: inside.as_bool(),
        }
    }

    fn selection_rects(&self, byte_start: usize, byte_end: usize) -> Vec<Rect> {
        let (start, end) = (self.unit_of_byte(byte_start), self.unit_of_byte(byte_end));
        if end <= start {
            return Vec::new();
        }
        let guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let mut boxes = vec![DWRITE_HIT_TEST_METRICS::default(); 16];
        let mut count = 0;
        // SAFETY: the buffer and count are valid; too small a buffer reports
        // the needed size in `count` and the call is repeated once with it.
        unsafe {
            let first = guard.0.HitTestTextRange(
                start,
                end - start,
                0.0,
                0.0,
                Some(&mut boxes),
                &mut count,
            );
            if first.is_err() && count as usize > boxes.len() {
                boxes.resize(count as usize, DWRITE_HIT_TEST_METRICS::default());
                let _ = guard.0.HitTestTextRange(
                    start,
                    end - start,
                    0.0,
                    0.0,
                    Some(&mut boxes),
                    &mut count,
                );
            }
        }
        boxes
            .iter()
            .take(count as usize)
            .map(|b| {
                Rect::new(
                    b.left.floor() as i32,
                    b.top.floor() as i32,
                    (b.left + b.width).ceil() as i32,
                    (b.top + b.height).ceil() as i32,
                )
            })
            .collect()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl Target {
    /// Draws a layout shaped by [`DwShaper`] with its top-left at `origin`.
    pub(crate) fn draw_layout(&self, layout: &DwLayout, origin: (f32, f32), color: Rgba) {
        let brush = self.solid(color);
        let guard = layout.inner.lock().unwrap_or_else(|e| e.into_inner());
        // SAFETY: inside a frame; the layout is locked for the call.
        unsafe {
            self.rt.DrawTextLayout(
                Vector2 {
                    X: origin.0,
                    Y: origin.1,
                },
                &guard.0,
                brush,
                D2D1_DRAW_TEXT_OPTIONS_NONE,
            )
        };
    }
}

/// The layout of a text DirectWrite could not shape: empty.
pub(crate) struct NoLayout;

impl TextLayout for NoLayout {
    fn width(&self) -> f32 {
        0.0
    }

    fn height(&self) -> f32 {
        0.0
    }

    fn hit_test_point(&self, _x: f32, _y: f32) -> TextHit {
        TextHit {
            byte_index: 0,
            inside: false,
        }
    }

    fn selection_rects(&self, _byte_start: usize, _byte_end: usize) -> Vec<Rect> {
        Vec::new()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
