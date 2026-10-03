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
use super::table::TableLayout;
use crate::model::{Document, TableSpan};

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
        let spans = doc.table_spans();
        let mut laid = 0;
        for _ in 0..ROUNDS {
            self.reposition(doc);
            let todo: Vec<usize> = self
                .visible(top, bottom)
                .filter(|&i| {
                    let p = &self.paras[i];
                    p.dirty || p.number != numbers[i] || !self.fits_at(i, p.y)
                })
                .collect();
            if todo.is_empty() || laid >= budget {
                break;
            }
            let mut done_to = 0;
            for index in todo.into_iter().take(budget - laid) {
                if index < done_to {
                    continue;
                }
                laid += 1;
                if let Some(span) = spans.iter().find(|t| t.paras.contains(&index)) {
                    self.lay_table_alone(doc, shaper, span, &numbers);
                    done_to = span.paras.end;
                    continue;
                }
                let (ctx, speculative) = match index.checked_sub(1).map(|i| &self.paras[i]) {
                    Some(prev) => (
                        FloatCtx::from_relative(&prev.exit, prev.bottom()),
                        prev.dirty || prev.speculative,
                    ),
                    None => (FloatCtx::default(), false),
                };
                let ctx = ctx.with_pages(self.pages);
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

    /// Lays out table `span` where the flow now puts it, before the tables
    /// and paragraphs above it are final (so speculatively if they are not).
    fn lay_table_alone(
        &mut self,
        doc: &Document,
        shaper: &dyn TextShaper,
        span: &TableSpan,
        numbers: &[Option<usize>],
    ) {
        let start = span.paras.start;
        let (y, ctx, speculative) = match start.checked_sub(1) {
            Some(prev) => {
                let (y, ctx) = self.after(prev);
                let p = &self.paras[prev];
                (y, ctx, p.dirty || p.speculative)
            }
            None => (0.0, FloatCtx::default().with_pages(self.pages), false),
        };
        self.lay_table(doc, shaper, span, y, &ctx, numbers);
        for i in span.paras.clone() {
            self.paras[i].speculative = speculative;
        }
        if let Some(next) = self.paras.get_mut(span.paras.end) {
            next.speculative = true;
        }
    }

    /// Gives every paragraph its position (estimating the heights of those
    /// not laid out) and the flow its height. A laid-out table moves as a
    /// whole; one that is not is stacked from its paragraphs' heights.
    pub(super) fn reposition(&mut self, doc: &Document) {
        let spans = doc.table_spans();
        self.tables
            .retain(|t| spans.iter().any(|s| s.paras == t.paras && s.id == t.id));
        let mut tables = spans.iter().peekable();
        let mut y = 0.0;
        let mut index = 0;
        while index < self.paras.len() {
            if let Some(span) = tables.next_if(|t| t.paras.start == index) {
                for i in span.paras.clone() {
                    let p = &self.paras[i];
                    if p.dirty && p.lines.is_empty() && p.height <= 0.0 {
                        let height = self.estimate(doc, i);
                        self.paras[i].height = height;
                    }
                }
                y = if self.table_of(index).is_some_and(|t| t.same_grid(span)) {
                    self.move_table(span, y);
                    self.table_of(index).map_or(y, TableLayout::bottom)
                } else {
                    self.place_estimated(span, y)
                };
                index = span.paras.end;
                continue;
            }
            let p = &self.paras[index];
            if p.dirty && p.lines.is_empty() && p.height <= 0.0 {
                let height = self.estimate(doc, index);
                self.paras[index].height = height;
            }
            self.paras[index].y = y;
            y += self.paras[index].height;
            index += 1;
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
