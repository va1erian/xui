#![forbid(unsafe_code)]

//! Viewport-first layout: lay out the paragraphs around a window first, give
//! the rest estimated heights, and finish them in budgeted slices later.
//!
//! A paragraph laid out before the ones above it are is *speculative*: it
//! assumed no floats entered it. When the in-order pass reaches it, it is kept
//! if the floats that really enter it are the same, and laid out again if not,
//! so the finished flow equals a full [`Layout::update`].

use xui_core::backend::TextShaper;

use super::floats::FloatCtx;
use super::flow::{Layout, list_numbers};
use super::resolve::{para_metrics, scale_for};
use crate::model::Document;

/// How many times a window is re-checked after layouts changed the heights
/// above it.
const ROUNDS: usize = 4;

impl Layout {
    /// Lays out the dirty paragraphs that intersect `top..bottom` (area
    /// pixels, by the current positions), at most `budget` of them, and gives
    /// the other dirty paragraphs estimated heights. Positions below shift as
    /// estimates are replaced by real heights, so call
    /// [`update_idle`](Layout::update_idle) until it reports no work left.
    pub fn update_around(
        &mut self,
        doc: &Document,
        shaper: &dyn TextShaper,
        top: f32,
        bottom: f32,
        budget: usize,
    ) {
        self.fit(doc);
        // An edit to an earlier item can renumber a clean visible one.
        let numbers = list_numbers(doc);
        let mut laid = 0;
        for _ in 0..ROUNDS {
            self.reposition(doc);
            let todo: Vec<usize> = self
                .visible(top, bottom)
                .filter(|&i| self.paras[i].dirty || self.paras[i].number != numbers[i])
                .collect();
            if todo.is_empty() || laid >= budget {
                break;
            }
            for index in todo.into_iter().take(budget - laid) {
                laid += 1;
                let (ctx, speculative) = match index.checked_sub(1).map(|i| &self.paras[i]) {
                    Some(prev) => (
                        FloatCtx::from_relative(&prev.exit, prev.bottom()),
                        prev.dirty || prev.speculative,
                    ),
                    None => (FloatCtx::default(), false),
                };
                let y = self.paras[index].y;
                let mut ctx = ctx;
                let old_exit = std::mem::take(&mut self.paras[index].exit);
                self.lay_one(doc, shaper, index, y, &mut ctx, numbers[index]);
                self.paras[index].speculative = speculative;
                // The floats leaving it changed, so the next paragraph's cached
                // layout may wrap around floats that are no longer there.
                if self.paras[index].exit != old_exit
                    && let Some(next) = self.paras.get_mut(index + 1)
                {
                    next.speculative = true;
                }
            }
        }
        self.reposition(doc);
    }

    /// Lays out at most `budget` of the paragraphs still waiting, in document
    /// order. Returns whether any work is left.
    pub fn update_idle(&mut self, doc: &Document, shaper: &dyn TextShaper, budget: usize) -> bool {
        self.fit(doc);
        if let Some(first) = self.first_pending() {
            self.run_from(doc, shaper, first, budget);
        }
        self.first_pending().is_some()
    }

    /// Whether every paragraph is laid out and verified.
    pub fn is_complete(&self) -> bool {
        self.first_pending().is_none()
    }

    /// Gives every paragraph its position (estimating the heights of those
    /// not laid out) and the flow its height.
    pub(super) fn reposition(&mut self, doc: &Document) {
        let mut y = 0.0;
        for index in 0..self.paras.len() {
            let p = &self.paras[index];
            if p.dirty && p.lines.is_empty() && p.height <= 0.0 {
                let height = self.estimate(doc, index);
                self.paras[index].height = height;
            }
            self.paras[index].y = y;
            y += self.paras[index].height;
        }
        let floats = match self.paras.last() {
            Some(last) if !last.dirty => {
                last.exit.iter().map(|e| e.rect.bottom).fold(0.0, f32::max)
            }
            _ => 0.0,
        };
        self.height = y + floats;
    }

    /// A guess at the height of paragraph `index` from its text length: half
    /// an em per character, 1.25 em per line.
    fn estimate(&self, doc: &Document, index: usize) -> f32 {
        let para = &doc.paragraphs()[index];
        let scale = scale_for(self.dpi);
        let m = para_metrics(doc.styles().para(para.style()), scale);
        let size = doc.styles().char(para.style_at(0)).size.0 * scale;
        let room = (self.width - m.left - m.right).max(size);
        let per_line = (room / (size * 0.5)).max(1.0);
        let lines = (para.text().chars().count() as f32 / per_line)
            .ceil()
            .max(1.0);
        m.before + lines * size * 1.25 + m.after
    }
}
