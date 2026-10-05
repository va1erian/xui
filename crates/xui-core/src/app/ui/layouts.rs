#![forbid(unsafe_code)]

//! [`Ui::layout_report`] and [`Ui::layout_rects`]: what the window's mounted
//! layouts placed, for a reader that cannot look at the screen.

use super::Ui;
use crate::geometry::Rect;

impl<M: 'static> Ui<M> {
    /// A text dump of every layout mounted on this window: its tree, where
    /// each group and widget landed (in device pixels, in the coordinates of
    /// the window or container it is mounted in) and a `!` warning for a node
    /// outside its parent (clipped), a visible widget of zero size, a text
    /// widget too narrow for its text, and siblings that overlap inside a
    /// row, column, grid or wrap (overlays and absolute layouts overlap on
    /// purpose).
    ///
    /// An agent or a test reads it instead of a screenshot; a clean layout
    /// has no line containing `!`.
    pub fn layout_report(&self) -> String {
        self.core.flush_layout();
        let mut out = String::new();
        for hook in self.core.layout_hooks() {
            hook.report(&mut out);
        }
        out
    }

    /// The rectangle of every group and widget the window-level layouts
    /// placed, in client coordinates: what a debug overlay outlines.
    pub fn layout_rects(&self) -> Vec<Rect> {
        self.core.flush_layout();
        let mut out = Vec::new();
        for hook in self.core.layout_hooks() {
            hook.rects(&mut out);
        }
        out
    }
}
