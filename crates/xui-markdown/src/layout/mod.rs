#![forbid(unsafe_code)]

//! The document laid out a block at a time, visible blocks first.
//!
//! Every block has a height: its real one once laid out, an estimate from its
//! length before. Block tops are a prefix sum over those heights, kept up to
//! date from the first block that changed. [`Layout::ensure`] lays out what a
//! view shows; [`Layout::work`] lays out the rest in time-boxed slices, so a
//! long document never delays the first paint.

mod block;
mod cache;
mod estimate;
mod flow;
mod geom;
mod line;
#[cfg(test)]
mod perf;
mod table;
#[cfg(test)]
mod tests;

use std::ops::Range;
use std::sync::Arc;
use std::time::{Duration, Instant};

use xui_core::backend::TextShaper;

use crate::model::Document;
use crate::style::MarkdownStyle;
use block::Context;
use cache::ShapeCache;
use estimate::{estimate, gap};
pub(crate) use geom::{BlockLayout, Ctx, Decor, FRect};
use table::Tables;

struct Slot {
    layout: Option<BlockLayout>,
    /// The laid-out height, or an estimate.
    height: f32,
    /// The space above the block.
    gap: f32,
}

/// A document's blocks laid out for one content width and DPI.
pub(crate) struct Layout {
    doc: Arc<Document>,
    style: MarkdownStyle,
    shaper: Box<dyn TextShaper>,
    cache: ShapeCache,
    tables: Tables,
    width: f32,
    dpi: u32,
    slots: Vec<Slot>,
    /// `tops[i]` is the top of block `i`'s text; `tops[n]` the end of the last.
    tops: Vec<f32>,
    /// The first index of `tops` that is out of date.
    stale: usize,
    /// No block before this one waits to be laid out.
    pending: usize,
}

impl Layout {
    pub fn new(
        doc: Arc<Document>,
        style: MarkdownStyle,
        shaper: Box<dyn TextShaper>,
        dpi: u32,
    ) -> Layout {
        let mut layout = Layout {
            tables: Tables::new(&doc),
            doc,
            style,
            shaper,
            cache: ShapeCache::default(),
            width: 0.0,
            dpi,
            slots: Vec::new(),
            tops: Vec::new(),
            stale: 0,
            pending: 0,
        };
        layout.reset();
        layout
    }

    pub fn doc(&self) -> &Arc<Document> {
        &self.doc
    }

    /// Shows another document.
    pub fn set_document(&mut self, doc: Arc<Document>) {
        self.tables = Tables::new(&doc);
        self.doc = doc;
        self.reset();
    }

    /// Changes the style, laying everything out again.
    pub fn set_style(&mut self, style: MarkdownStyle) {
        self.style = style;
        self.cache.clear();
        self.reset();
    }

    /// Lays out for `width` device pixels of text at `dpi`. A new width or
    /// DPI drops every block's layout and estimates the heights again for it,
    /// so the scrollbar is right before the blocks are laid out again.
    pub fn resize(&mut self, width: f32, dpi: u32) {
        if width == self.width && dpi == self.dpi {
            return;
        }
        if dpi != self.dpi {
            self.cache.clear();
        }
        self.width = width;
        self.dpi = dpi;
        self.reset();
    }

    /// Whether a width has been given, so laying out means something.
    pub fn sized(&self) -> bool {
        self.width > 0.0
    }

    fn reset(&mut self) {
        let scale = self.dpi as f32 / 96.0;
        let doc = Arc::clone(&self.doc);
        self.slots = (0..doc.blocks().len())
            .map(|i| Slot {
                layout: None,
                height: estimate(&doc, &doc.blocks()[i], &self.style, scale, self.width),
                gap: gap(&doc, i, &self.style, scale),
            })
            .collect();
        self.tables.clear();
        self.tops = vec![0.0; self.slots.len() + 1];
        self.stale = 0;
        self.pending = 0;
    }

    fn refresh_tops(&mut self) {
        if self.stale > self.slots.len() {
            return;
        }
        let mut y = if self.stale == 0 {
            0.0
        } else {
            self.tops[self.stale - 1] + self.slots[self.stale - 1].height
        };
        for i in self.stale..self.slots.len() {
            y += self.slots[i].gap;
            self.tops[i] = y;
            y += self.slots[i].height;
        }
        if let Some(end) = self.tops.last_mut() {
            *end = y;
        }
        self.stale = self.slots.len() + 1;
    }

    /// The top of block `index`'s text, from the top of the content.
    pub fn top(&mut self, index: usize) -> f32 {
        self.refresh_tops();
        self.tops[index.min(self.slots.len())]
    }

    /// The height of all blocks.
    pub fn height(&mut self) -> f32 {
        self.refresh_tops();
        self.tops.last().copied().unwrap_or(0.0)
    }

    /// The blocks that overlap `y0..y1`.
    pub fn visible(&mut self, y0: f32, y1: f32) -> Range<usize> {
        self.refresh_tops();
        let n = self.slots.len();
        let first = self.tops[..n]
            .partition_point(|&top| top <= y0)
            .saturating_sub(1);
        let last = self.tops[..n].partition_point(|&top| top < y1);
        first..last.max(first)
    }

    /// The block whose area (its gap included) holds `y`.
    pub fn block_at(&mut self, y: f32) -> usize {
        self.refresh_tops();
        let n = self.slots.len();
        self.tops[..n]
            .partition_point(|&top| top <= y)
            .saturating_sub(1)
    }

    fn lay_out(&mut self, index: usize) {
        if self.slots[index].layout.is_some() {
            return;
        }
        let around = Context {
            gap: self.slots[index].gap,
            joined_quotes: match index {
                0 => 0,
                _ => self.doc.blocks()[index - 1]
                    .quote
                    .min(self.doc.blocks()[index].quote),
            },
        };
        let doc = Arc::clone(&self.doc);
        let mut ctx = Ctx {
            doc: &doc,
            style: &self.style,
            shaper: self.shaper.as_ref(),
            cache: &mut self.cache,
            dpi: self.dpi,
        };
        let laid = block::layout(&mut ctx, index, self.width, around, &mut self.tables);
        let slot = &mut self.slots[index];
        if slot.height != laid.height {
            self.stale = self.stale.min(index + 1);
        }
        slot.height = laid.height;
        slot.layout = Some(laid);
    }

    /// Lays out every block that overlaps `y0..y1`, settling the estimates
    /// above them. Returns the blocks.
    pub fn ensure(&mut self, y0: f32, y1: f32) -> Range<usize> {
        // Laying a block out corrects its estimate, which moves the blocks
        // below and can bring new ones into view: repeat until every block
        // in view is laid out. Each pass lays out at least one more block,
        // so this ends.
        loop {
            let range = self.visible(y0, y1);
            let mut fresh = false;
            for i in range.clone() {
                fresh |= self.slots[i].layout.is_none();
                self.lay_out(i);
            }
            if !fresh {
                return range;
            }
        }
    }

    /// Lays out blocks not yet laid out, in order, for about `budget`.
    /// Returns whether any remain.
    pub fn work(&mut self, budget: Duration) -> bool {
        let start = Instant::now();
        while self.pending < self.slots.len() {
            self.lay_out(self.pending);
            self.pending += 1;
            if start.elapsed() >= budget {
                break;
            }
        }
        self.pending < self.slots.len()
    }

    /// Whether every block is laid out.
    pub fn done(&self) -> bool {
        self.pending >= self.slots.len()
    }

    /// Block `index`'s layout, if it has one.
    pub fn block(&self, index: usize) -> Option<&BlockLayout> {
        self.slots.get(index)?.layout.as_ref()
    }

    /// The link at `(x, y)` (content pixels, `x` from the text's left edge).
    pub fn link_at(&mut self, x: f32, y: f32) -> Option<u32> {
        let index = self.block_at(y);
        let top = self.top(index);
        let laid = self.block(index)?;
        laid.links
            .iter()
            .find(|(rect, _)| rect.contains(x, y - top))
            .map(|&(_, link)| link)
    }
}
