#![forbid(unsafe_code)]

//! The selection: an anchor and a focus in the document, the word or block
//! around a position, and the rectangles that show it, kept between paints.

use std::ops::Range;

use unicode_segmentation::UnicodeSegmentation;

use crate::layout::{FRect, Layout};

/// A selection from `anchor` (where it started) to `focus` (where it ends
/// now), as byte offsets into [`Document::text`](crate::Document::text).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Selection {
    pub anchor: u32,
    pub focus: u32,
}

impl Selection {
    pub fn at(pos: u32) -> Selection {
        Selection {
            anchor: pos,
            focus: pos,
        }
    }

    pub fn of(range: Range<u32>) -> Selection {
        Selection {
            anchor: range.start,
            focus: range.end,
        }
    }

    /// The selected bytes, in document order.
    pub fn range(&self) -> Range<u32> {
        self.anchor.min(self.focus)..self.anchor.max(self.focus)
    }

    pub fn is_empty(&self) -> bool {
        self.anchor == self.focus
    }
}

/// The word (or run of spaces or punctuation) around `pos` in block `index`.
pub(crate) fn word_at(layout: &Layout, index: usize, pos: u32) -> Range<u32> {
    let doc = layout.doc();
    let block = &doc.blocks()[index];
    let text = doc.block_text(block);
    let local = (pos.saturating_sub(block.text.start) as usize).min(text.len());
    let mut found = local..local;
    for (at, word) in text.split_word_bound_indices() {
        let end = at + word.len();
        // A position between two segments belongs to the one after it,
        // unless it ends the text.
        if (at..end).contains(&local) || (end == text.len() && local == end) {
            found = at..end;
            break;
        }
    }
    block.text.start + found.start as u32..block.text.start + found.end as u32
}

/// The whole text of block `index`.
pub(crate) fn block_text(layout: &Layout, index: usize) -> Range<u32> {
    layout.doc().blocks()[index].text.clone()
}

/// The selection's rectangles per block, worked out when a block is first
/// painted and kept until the selection or that block's layout changes, so
/// repainting a selection allocates nothing.
#[derive(Default)]
pub(crate) struct RectCache {
    selection: Range<u32>,
    /// Each block's rectangles and the generation of the layout they came
    /// from.
    blocks: Vec<(usize, u64, Vec<FRect>)>,
}

impl RectCache {
    /// Makes sure block `index`'s rectangles for `selection` are cached.
    pub fn prepare(&mut self, layout: &Layout, selection: &Range<u32>, index: usize) {
        if self.selection != *selection {
            self.selection = selection.clone();
            self.blocks.clear();
        }
        let made = layout.generation(index);
        match self.blocks.iter_mut().find(|(i, _, _)| *i == index) {
            Some((_, at, _)) if *at == made => {}
            Some(entry) => *entry = (index, made, layout.selection_rects(index, selection)),
            None => self
                .blocks
                .push((index, made, layout.selection_rects(index, selection))),
        }
    }

    /// Block `index`'s cached rectangles (empty when none are cached).
    pub fn get(&self, index: usize) -> &[FRect] {
        self.blocks
            .iter()
            .find(|(i, _, _)| *i == index)
            .map_or(&[], |(_, _, rects)| rects)
    }
}
