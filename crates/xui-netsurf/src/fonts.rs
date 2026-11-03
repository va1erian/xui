#![forbid(unsafe_code)]

//! Text measuring for NetSurf's layout, over the backend's portable
//! [`TextShaper`] at 96 dpi (one CSS pixel per device-independent pixel, the
//! unit `xui-litehtml` lays out in too), so both engines measure with the same
//! fonts.
//!
//! NetSurf asks three questions: a string's width, the character boundary
//! nearest an x (caret placement), and where to break a string to fit a width
//! (line breaking). The last two walk per-character advances, as NetSurf's own
//! framebuffer frontend does.

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

use xui_core::Dip;
use xui_core::backend::{FontSpec, TextShaper};

/// The dpi text is shaped at: one device pixel per CSS pixel.
const LAYOUT_DPI: u32 = 96;
/// Cached widths per font before the cache is dropped.
const MAX_CACHED: usize = 8192;

/// A font as NetSurf asks for it.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct FontReq {
    /// The family to ask the shaper for; empty for its default family.
    pub(crate) family: String,
    /// The em size in CSS pixels.
    pub(crate) size: f32,
    /// 100 to 900.
    pub(crate) weight: u16,
    /// Italic or oblique.
    pub(crate) italic: bool,
}

impl FontReq {
    /// The family to use for the first family the CSS named and NetSurf's
    /// generic one (see `families`).
    pub(crate) fn family_for(named: Option<&str>, generic: i32) -> String {
        crate::families::resolve(named, generic)
    }

    fn key(&self) -> (String, u32, u16, bool) {
        (
            self.family.clone(),
            self.size.to_bits(),
            self.weight,
            self.italic,
        )
    }

    fn spec(&self) -> FontSpec {
        let spec = FontSpec::new(Dip(self.size.max(1.0)))
            .weight(self.weight)
            .italic(self.italic);
        // Empty means the shaper's default family, as in `xui-litehtml`.
        if self.family.is_empty() {
            spec
        } else {
            spec.family(self.family.as_str())
        }
    }
}

#[derive(Default)]
struct FontCache {
    widths: HashMap<String, i32>,
    chars: HashMap<char, f32>,
    ascent: Option<f32>,
}

/// The measurer NetSurf's layout calls into, with per-font caches. It lives
/// on the engine thread.
pub(crate) struct Fonts {
    shaper: Arc<dyn TextShaper>,
    cache: RefCell<HashMap<(String, u32, u16, bool), FontCache>>,
}

impl Fonts {
    pub(crate) fn new(shaper: Arc<dyn TextShaper>) -> Fonts {
        Fonts {
            shaper,
            cache: RefCell::default(),
        }
    }

    fn with_cache<R>(&self, font: &FontReq, f: impl FnOnce(&mut FontCache) -> R) -> R {
        let mut cache = self.cache.borrow_mut();
        let entry = cache.entry(font.key()).or_default();
        if entry.widths.len() > MAX_CACHED {
            entry.widths.clear();
        }
        f(entry)
    }

    fn shaped_width(&self, font: &FontReq, text: &str) -> f32 {
        self.shaper
            .layout(text, &font.spec(), f32::INFINITY, LAYOUT_DPI)
            .width()
    }

    /// The width of `text`, shaped as a whole (kerning included).
    pub(crate) fn width(&self, font: &FontReq, text: &str) -> i32 {
        if text.is_empty() {
            return 0;
        }
        if let Some(w) = self.with_cache(font, |c| c.widths.get(text).copied()) {
            return w;
        }
        let w = self.shaped_width(font, text).round() as i32;
        self.with_cache(font, |c| c.widths.insert(text.to_string(), w));
        w
    }

    /// The distance from a line box's top to the baseline. The portable
    /// shaper reports only a line height; the ascent is the same proportional
    /// estimate `xui-litehtml` uses, so both engines place text alike.
    pub(crate) fn ascent(&self, font: &FontReq) -> f32 {
        if let Some(a) = self.with_cache(font, |c| c.ascent) {
            return a;
        }
        let height = self
            .shaper
            .layout("Hg", &font.spec(), f32::INFINITY, LAYOUT_DPI)
            .height();
        let ascent = height * 0.8;
        self.with_cache(font, |c| c.ascent = Some(ascent));
        ascent
    }

    /// The line height the shaper gives `font`.
    pub(crate) fn line_height(&self, font: &FontReq) -> f32 {
        self.ascent(font) / 0.8
    }

    fn advance(&self, font: &FontReq, ch: char) -> f32 {
        if let Some(a) = self.with_cache(font, |c| c.chars.get(&ch).copied()) {
            return a;
        }
        let mut buf = [0u8; 4];
        let a = self.shaped_width(font, ch.encode_utf8(&mut buf));
        self.with_cache(font, |c| c.chars.insert(ch, a));
        a
    }

    /// The character boundary nearest `x`: its byte offset and x.
    pub(crate) fn position(&self, font: &FontReq, text: &str, x: i32) -> (usize, i32) {
        let x = x as f32;
        let mut prev = 0.0f32;
        for (i, ch) in text.char_indices() {
            let next = prev + self.advance(font, ch);
            if next > x {
                // The nearer of the boundaries either side of `x`.
                return if (next - x) < (x - prev) {
                    (i + ch.len_utf8(), next.round() as i32)
                } else {
                    (i, prev.round() as i32)
                };
            }
            prev = next;
        }
        (text.len(), prev.round() as i32)
    }

    /// Where to break `text` to fit `x`: the last space before the width runs
    /// past `x`, else the first space after (an unbreakable run overflows),
    /// else the end; and the width up to there.
    pub(crate) fn split(&self, font: &FontReq, text: &str, x: i32) -> (usize, i32) {
        split_at(text, x as f32, |ch| self.advance(font, ch))
    }
}

/// [`Fonts::split`] over any advance function, so it can be tested alone.
fn split_at(text: &str, x: f32, mut advance: impl FnMut(char) -> f32) -> (usize, i32) {
    let mut width = 0.0f32;
    let mut space: Option<(usize, f32)> = None;
    for (i, ch) in text.char_indices() {
        if ch == ' ' {
            space = Some((i, width));
        }
        width += advance(ch);
        if width > x
            && let Some((at, w)) = space
            && at > 0
        {
            return (at, w.round() as i32);
        }
    }
    (text.len(), width.round() as i32)
}

#[cfg(test)]
mod tests {
    use super::split_at;

    fn mono(_: char) -> f32 {
        10.0
    }

    #[test]
    fn split_breaks_at_the_last_space_that_fits() {
        // "aaa bbb ccc": the second space is at 7, width 70.
        assert_eq!(split_at("aaa bbb ccc", 85.0, mono), (7, 70));
    }

    #[test]
    fn split_keeps_an_unbreakable_word_whole() {
        assert_eq!(split_at("aaaaaaaa bb", 30.0, mono), (8, 80));
    }

    #[test]
    fn split_returns_the_whole_string_when_it_fits() {
        assert_eq!(split_at("aa bb", 500.0, mono), (5, 50));
    }
}
