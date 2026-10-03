#![forbid(unsafe_code)]

//! A bounded cache of shaped words, keyed by (text, character style, dpi), so
//! retyping a paragraph reshapes only the words that changed.

use std::collections::HashMap;
use std::sync::Arc;

use xui_core::backend::{FontSpec, TextLayout, TextShaper};

use crate::model::{BlockKind, CharStyleId};

/// Cached words before the oldest are evicted.
const CAPACITY: usize = 20_000;

/// Everything cached for one (style, block kind, dpi).
pub(crate) struct StyleEntry {
    spec: FontSpec,
    dpi: u32,
    words: HashMap<Box<str>, (Arc<dyn TextLayout>, u64)>,
    space: Option<f32>,
    strut: Option<(f32, f32)>,
}

impl StyleEntry {
    /// The font the entry shapes with.
    pub fn spec(&self) -> &FontSpec {
        &self.spec
    }

    /// Shapes `text` unwrapped, or returns the cached layout.
    pub fn word(&mut self, shaper: &dyn TextShaper, text: &str, tick: u64) -> Arc<dyn TextLayout> {
        if let Some((layout, used)) = self.words.get_mut(text) {
            *used = tick;
            return Arc::clone(layout);
        }
        let layout: Arc<dyn TextLayout> =
            Arc::from(shaper.layout(text, &self.spec, f32::INFINITY, self.dpi));
        self.words.insert(text.into(), (Arc::clone(&layout), tick));
        layout
    }

    /// The width of one space. Measured as the difference between two words
    /// with and without a space so a backend that drops trailing whitespace
    /// from its width still answers correctly.
    pub fn space_width(&mut self, shaper: &dyn TextShaper) -> f32 {
        if let Some(width) = self.space {
            return width;
        }
        let width = |text: &str| {
            shaper
                .layout(text, &self.spec, f32::INFINITY, self.dpi)
                .width()
        };
        let unit = (width("a a") - width("aa")).max(1.0);
        self.space = Some(unit);
        unit
    }

    /// The (ascent, descent) of an empty line in the style.
    pub fn strut(&mut self, shaper: &dyn TextShaper) -> (f32, f32) {
        if let Some(strut) = self.strut {
            return strut;
        }
        let layout = shaper.layout("x", &self.spec, f32::INFINITY, self.dpi);
        let ascent = layout.baseline();
        let strut = (ascent, (layout.height() - ascent).max(0.0));
        self.strut = Some(strut);
        strut
    }
}

/// Shaped words by style.
pub(crate) struct ShapeCache {
    styles: HashMap<(u32, CharStyleId, BlockKind), StyleEntry>,
    tick: u64,
    capacity: usize,
}

impl Default for ShapeCache {
    fn default() -> ShapeCache {
        ShapeCache {
            styles: HashMap::new(),
            tick: 0,
            capacity: CAPACITY,
        }
    }
}

impl ShapeCache {
    /// Sets how many words are kept before the oldest are evicted.
    #[cfg(test)]
    pub fn set_capacity(&mut self, capacity: usize) {
        self.capacity = capacity;
    }

    /// Marks the start of a unit of work (a paragraph); words used since are
    /// kept when the cache is trimmed.
    pub fn begin(&mut self) -> u64 {
        self.tick += 1;
        self.tick
    }

    /// The entry for a style, creating it from `spec` the first time.
    pub fn entry(
        &mut self,
        dpi: u32,
        style: CharStyleId,
        kind: BlockKind,
        spec: impl FnOnce() -> FontSpec,
    ) -> &mut StyleEntry {
        self.styles
            .entry((dpi, style, kind))
            .or_insert_with(|| StyleEntry {
                spec: spec(),
                dpi,
                words: HashMap::new(),
                space: None,
                strut: None,
            })
    }

    /// Evicts the least recently used words when the cache is over capacity.
    pub fn trim(&mut self) {
        let mut total: usize = self.styles.values().map(|e| e.words.len()).sum();
        let mut keep = 1024;
        while total > self.capacity && keep > 0 {
            let oldest = self.tick.saturating_sub(keep);
            for entry in self.styles.values_mut() {
                entry.words.retain(|_, (_, used)| *used > oldest);
            }
            total = self.styles.values().map(|e| e.words.len()).sum();
            keep /= 2;
        }
        if total > self.capacity {
            self.clear();
        }
    }

    /// Forgets everything (the document changed its style table).
    pub fn clear(&mut self) {
        self.styles.clear();
    }

    /// How many words are cached.
    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.styles.values().map(|e| e.words.len()).sum()
    }
}
