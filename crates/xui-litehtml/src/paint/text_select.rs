//! Hit-testing and selection over a [`Painter`](super::Painter)'s shaped text,
//! split from `paint.rs` so both files stay under the size limit.

use crate::geom::{Point, Rect};
use crate::list::{DisplayList, FontKey};
use crate::selection::{Selection, TextPos};
use crate::text::Font;
use crate::text_runs::{TextRun, TextRunTable};

use super::Painter;

impl Painter {
    /// Resolves the font for `key`, for the widget's hit-testing and selection
    /// boxes (which rebuild a the shaper layout per run).
    pub(crate) fn resolve_font(&mut self, list: &DisplayList, key: FontKey) -> Option<Font> {
        self.font(list, key)
    }

    /// The caret nearest `doc`, using the shaper hit-testing for the
    /// character boundary (accurate for right-to-left and complex text, where
    /// the run table's left-to-right `offsets` are not).
    pub fn caret_at(
        &mut self,
        list: &DisplayList,
        runs: &TextRunTable,
        doc: Point,
    ) -> Option<TextPos> {
        let run = runs.nearest_run(doc)?;
        let text_run = &runs.runs[run];
        let font = self.resolve_font(list, text_run.font)?;
        let layout = font.layout(&text_run.text);
        let hit = layout.hit_test_point(doc.x - text_run.rect.left, 0.0);
        let byte = hit.byte_index.min(text_run.text.len());
        let ch = text_run.text[..byte].chars().count();
        Some(TextPos { run, ch })
    }

    /// The highlight boxes of `sel`, in document coordinates, from the shaper's
    /// per-run selection rects, merged across words the way the run table's own
    /// offsets-based selection does (so a whole line highlights as one box).
    pub fn selection_rects(
        &mut self,
        list: &DisplayList,
        runs: &TextRunTable,
        sel: &Selection,
    ) -> Vec<Rect> {
        let (start, end) = sel.ordered();
        let mut out: Vec<Rect> = Vec::new();
        let Some(last_run) = runs.runs.len().checked_sub(1) else {
            return out;
        };
        for i in start.run..=end.run.min(last_run) {
            let run = &runs.runs[i];
            let from = if i == start.run { start.ch } else { 0 };
            let to = (if i == end.run {
                end.ch
            } else {
                run.char_count()
            })
            .min(run.char_count());
            if from >= to {
                continue;
            }
            let rect = run_rect(self, list, run, from, to).unwrap_or_else(|| Rect {
                left: run.rect.left + run.offsets[from],
                top: run.rect.top,
                right: run.rect.left + run.offsets[to],
                bottom: run.rect.bottom,
            });
            let extends_last = out.last().is_some_and(|last| {
                (last.center().y - rect.center().y).abs() < rect.height() * 0.5
                    && (rect.left - last.right).abs() < 2.0
            });
            if extends_last {
                let last = out.last_mut().unwrap();
                *last = last.union(rect);
            } else if !run.text.trim().is_empty() {
                out.push(rect);
            }
        }
        out
    }
}

/// The horizontal highlight extent of `run[from..to]`, from the shaper's
/// selection rects (the vertical is the run's own box, matching the highlight
/// of the egui widget). Falls back to `None` when the font cannot be resolved.
fn run_rect(
    painter: &mut Painter,
    list: &DisplayList,
    run: &TextRun,
    from: usize,
    to: usize,
) -> Option<Rect> {
    let font = painter.resolve_font(list, run.font)?;
    let layout = font.layout(&run.text);
    let from_byte = char_to_byte(&run.text, from);
    let to_byte = char_to_byte(&run.text, to);
    let boxes = layout.selection_rects(from_byte, to_byte);
    if boxes.is_empty() {
        return None;
    }
    let left = boxes
        .iter()
        .map(|b| b.left as f32)
        .fold(f32::INFINITY, f32::min);
    let right = boxes
        .iter()
        .map(|b| b.right as f32)
        .fold(f32::NEG_INFINITY, f32::max);
    Some(Rect {
        left: run.rect.left + left,
        top: run.rect.top,
        right: run.rect.left + right,
        bottom: run.rect.bottom,
    })
}

/// The byte offset of the `ch`-th character (clamped to the text's end).
fn char_to_byte(text: &str, ch: usize) -> usize {
    text.char_indices().nth(ch).map_or(text.len(), |(i, _)| i)
}
