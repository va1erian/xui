#![forbid(unsafe_code)]

//! The editor's periodic work, run on one timer: the caret blink, autoscroll
//! while dragging outside the view, and finishing the layout in idle slices.
//! Kept as a plain method so it can be driven by hand.

use super::state::State;

/// Timer ticks per caret blink phase.
pub(crate) const BLINK_TICKS: u32 = 10;
/// Paragraphs laid out per tick while the layout is catching up.
const IDLE_SLICE: usize = 24;

impl State {
    /// One timer tick. Returns whether the view must repaint.
    pub fn tick(&mut self) -> bool {
        self.ticks = self.ticks.wrapping_add(1);
        let mut repaint = self.autoscroll();
        repaint |= self.finish_layout_slice();
        if self.focused && self.ticks.is_multiple_of(BLINK_TICKS) {
            self.caret_on = !self.caret_on;
            repaint = true;
        }
        repaint
    }

    /// Lays out a slice of the paragraphs still waiting. The paragraph at the
    /// top of the view keeps its place on screen as the heights above it turn
    /// from estimates into real ones.
    fn finish_layout_slice(&mut self) -> bool {
        if self.layout.is_complete() {
            return false;
        }
        let shift = self.shift() as f32;
        let anchor = self.layout.visible(shift, shift + 1.0).start;
        let offset = self.layout.paragraphs().get(anchor).map(|p| shift - p.y);
        self.layout
            .update_idle(&self.ed.doc, self.shaper.as_ref(), IDLE_SLICE);
        if let Some(offset) = offset {
            let moved = self.layout.paragraphs()[anchor].y + offset - shift;
            self.scroll = (self.scroll + moved).clamp(0.0, self.max_scroll());
        }
        true
    }
}
