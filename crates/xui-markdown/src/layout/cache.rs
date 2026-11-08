#![forbid(unsafe_code)]

//! The shape cache: words shaped once per font and reused.
//!
//! Prose repeats its words, so a mixed-style paragraph mostly reuses layouts
//! made for earlier ones. Lookups borrow the word as `&str`, so a hit
//! allocates nothing.

use std::collections::HashMap;
use std::sync::Arc;

use xui_core::backend::{TextLayout, TextShaper};

use crate::style::{FontKey, MarkdownStyle};

/// Past this many words the cache starts over, bounding its memory on a huge
/// document.
const MAX_ENTRIES: usize = 20_000;

/// A word shaped on one line.
#[derive(Clone)]
pub(crate) struct Shaped {
    pub layout: Arc<dyn TextLayout>,
    pub width: f32,
    pub height: f32,
    pub baseline: f32,
}

impl Shaped {
    pub fn new(layout: Box<dyn TextLayout>) -> Shaped {
        Shaped {
            width: layout.width(),
            height: layout.height(),
            baseline: layout.baseline(),
            layout: Arc::from(layout),
        }
    }
}

/// A font's line metrics in device pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Metrics {
    pub height: f32,
    pub baseline: f32,
    /// The advance of one space.
    pub space: f32,
}

#[derive(Default)]
pub(crate) struct ShapeCache {
    words: HashMap<FontKey, HashMap<Box<str>, Shaped>>,
    metrics: HashMap<FontKey, Metrics>,
    entries: usize,
}

impl ShapeCache {
    /// Forgets everything, for a new DPI or style.
    pub fn clear(&mut self) {
        self.words.clear();
        self.metrics.clear();
        self.entries = 0;
    }

    /// `text` shaped on one line in `font`.
    pub fn word(
        &mut self,
        shaper: &dyn TextShaper,
        style: &MarkdownStyle,
        font: FontKey,
        text: &str,
        dpi: u32,
    ) -> Shaped {
        if let Some(hit) = self.words.get(&font).and_then(|words| words.get(text)) {
            return hit.clone();
        }
        if self.entries >= MAX_ENTRIES {
            self.words.clear();
            self.entries = 0;
        }
        let shaped = Shaped::new(shaper.layout(text, &style.spec(font), f32::INFINITY, dpi));
        self.entries += 1;
        self.words
            .entry(font)
            .or_default()
            .insert(text.into(), shaped.clone());
        shaped
    }

    /// The line height, baseline and space advance of `font`.
    pub fn metrics(
        &mut self,
        shaper: &dyn TextShaper,
        style: &MarkdownStyle,
        font: FontKey,
        dpi: u32,
    ) -> Metrics {
        if let Some(metrics) = self.metrics.get(&font) {
            return *metrics;
        }
        let spec = style.spec(font);
        // A lone or trailing space may measure zero (DirectWrite leaves
        // trailing whitespace out of a line's width), so a space is measured
        // between two letters.
        let spaced = shaper.layout("n n", &spec, f32::INFINITY, dpi);
        let tight = shaper.layout("nn", &spec, f32::INFINITY, dpi);
        let metrics = Metrics {
            height: spaced.height(),
            baseline: spaced.baseline(),
            space: (spaced.width() - tight.width()).max(0.0),
        };
        self.metrics.insert(font, metrics);
        metrics
    }
}
