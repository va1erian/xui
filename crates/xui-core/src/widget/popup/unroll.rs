#![forbid(unsafe_code)]

//! [`Unroll`]: a popup node that opens and closes by growing and shrinking.
//!
//! There is no per-node opacity or transform, so the effect is the node's own
//! bounds: its height runs between [`START`] and the full height, hanging from
//! the top edge. A popup is placed below its anchor, or clamped into the client
//! area by shifting it, never flipped, so the top edge is always the one it
//! hangs from. The popup's painter lays its rows out from the node's top, so
//! growing the node reveals them.

use std::cell::Cell;
use std::rc::Rc;

use crate::app::Ui;
use crate::backend::WidgetId;
use crate::geometry::Rect;
use crate::motion::{Animated, Motion};
use crate::units::Dip;

/// The height a popup starts unrolling from, and rolls back to before it hides.
const START: Dip = Dip(8.0);

/// The unrolling of one popup node.
///
/// It holds one value per node: starting either direction retargets it from
/// where it is, so a popup reversed mid-flight turns round instead of jumping,
/// and the older animation of the node is replaced.
pub(crate) struct Unroll {
    progress: Rc<Animated>,
    /// The bounds the popup unrolls to.
    full: Cell<Rect>,
    /// The node is visible: open, or rolling shut.
    shown: Cell<bool>,
    /// The node is rolling shut and hides when it lands.
    closing: Cell<bool>,
}

impl Unroll {
    /// A hidden popup.
    pub(crate) fn new() -> Rc<Unroll> {
        Rc::new(Unroll {
            progress: Rc::new(Animated::new(0.0)),
            full: Cell::new(Rect::default()),
            shown: Cell::new(false),
            closing: Cell::new(false),
        })
    }

    /// Shows node `id` and unrolls it to `full`; with `animate` false, or
    /// animations off, it is shown at `full` at once. The node has its
    /// starting bounds before it is shown, because a native backend paints a
    /// popup synchronously inside `set_visible`. A popup still rolling shut is
    /// reopened from the height it has reached.
    pub(crate) fn open<M: 'static>(
        self: &Rc<Self>,
        ui: &Ui<M>,
        id: WidgetId,
        full: Rect,
        animate: bool,
    ) {
        self.full.set(full);
        self.closing.set(false);
        if !(animate && ui.animations()) {
            self.progress.jump(1.0);
            self.shown.set(true);
            ui.apply_moves(&[(id, full)]);
            ui.set_visible(id, true);
            return;
        }
        if !self.shown.replace(true) {
            self.progress.jump(0.0);
        }
        let from = self.progress.value(ui.frame_time());
        ui.apply_moves(&[(id, self.rect(ui, from))]);
        ui.set_visible(id, true);
        self.run(ui, id, 1.0, Motion::REVEAL, || {});
    }

    /// Rolls node `id` shut, then hides it and calls `hidden`; all at once when
    /// animations are off. A popup that is not shown only calls `hidden`.
    pub(crate) fn close<M: 'static>(
        self: &Rc<Self>,
        ui: &Ui<M>,
        id: WidgetId,
        hidden: impl Fn() + 'static,
    ) {
        if !self.shown.get() {
            hidden();
            return;
        }
        self.closing.set(true);
        self.run(ui, id, 0.0, Motion::EXIT, hidden);
    }

    /// Hides node `id` at once, dropping any unrolling in flight.
    pub(crate) fn hide<M: 'static>(&self, ui: &Ui<M>, id: WidgetId) {
        self.progress.jump(0.0);
        self.closing.set(false);
        if self.shown.replace(false) {
            ui.set_visible(id, false);
        }
    }

    fn run<M: 'static>(
        self: &Rc<Self>,
        ui: &Ui<M>,
        id: WidgetId,
        to: f32,
        motion: Motion,
        hidden: impl Fn() + 'static,
    ) {
        let this = Rc::clone(self);
        let handle = ui.clone();
        ui.animate_with(id, &self.progress, to, motion, move |progress| {
            this.step(&handle, id, progress, &hidden);
        });
    }

    /// Applies one reading of the progress. The reading that ends a close hides
    /// the node; one that arrives after the popup was hidden by other means
    /// does nothing.
    fn step<M: 'static>(&self, ui: &Ui<M>, id: WidgetId, progress: f32, hidden: &dyn Fn()) {
        if !self.shown.get() {
            return;
        }
        if progress <= 0.0 && self.closing.get() {
            self.closing.set(false);
            self.shown.set(false);
            ui.set_visible(id, false);
            hidden();
            return;
        }
        ui.apply_moves(&[(id, self.rect(ui, progress))]);
    }

    fn rect<M: 'static>(&self, ui: &Ui<M>, progress: f32) -> Rect {
        unrolled(self.full.get(), progress, START.to_px(ui.dpi()).value())
    }
}

/// `full` cut to `progress` of the way from `start` high to its full height,
/// from its top edge; exactly `full` at `1.0`.
fn unrolled(full: Rect, progress: f32, start: i32) -> Rect {
    let height = full.height();
    let start = start.min(height);
    let shown = if progress >= 1.0 {
        height
    } else {
        start + ((height - start) as f32 * progress.max(0.0)).round() as i32
    };
    Rect::new(full.left, full.top, full.right, full.top + shown)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unrolled_popup_grows_from_its_top_edge() {
        let full = Rect::new(10, 20, 110, 220);
        assert_eq!(unrolled(full, 0.0, 8), Rect::new(10, 20, 110, 28));
        assert_eq!(unrolled(full, 0.5, 8), Rect::new(10, 20, 110, 124));
        assert_eq!(unrolled(full, 1.0, 8), full);
        assert_eq!(unrolled(full, -1.0, 8), unrolled(full, 0.0, 8));
        let short = Rect::new(0, 0, 50, 5);
        assert_eq!(unrolled(short, 0.0, 8), short, "never taller than itself");
    }
}
