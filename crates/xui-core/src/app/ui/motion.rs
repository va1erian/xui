#![forbid(unsafe_code)]

//! Animation through the [`Ui`]: the system's reduced-motion preference, the
//! application's switch, and starting an [`Animated`] value.

use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

use super::Ui;
use crate::backend::WidgetId;
use crate::motion::{Animated, Motion};

impl<M: 'static> Ui<M> {
    /// Whether the user has asked the system to reduce motion.
    ///
    /// Anything that animates must check this when it starts: when it is `true`
    /// motion resolves to a plain fade or an instant change, and nothing loops.
    /// It is read live from the backend, so a change made while the app runs
    /// applies to the next animation. A backend with no way to read the setting
    /// reports `false`; see
    /// [`Backend::prefers_reduced_motion`](crate::backend::Backend::prefers_reduced_motion)
    /// for which do. [`Ui::animate`] checks it itself.
    pub fn prefers_reduced_motion(&self) -> bool {
        self.core.backend().prefers_reduced_motion()
    }

    /// Turns the window's animations on (the default) or off. Off, every
    /// change started from then on is instant, as it is under
    /// [`Ui::prefers_reduced_motion`]; a value already moving finishes.
    pub fn set_animations(&self, on: bool) {
        self.core.motion().set_enabled(on);
    }

    /// Whether a change started through this handle now would animate: the
    /// application left animations on, the system does not ask for reduced
    /// motion, the handle is not in design mode and the backend has a clock.
    pub fn animations(&self) -> bool {
        self.core.motion().enabled()
            && !self.is_design_mode()
            && !self.prefers_reduced_motion()
            && self.core.backend().now().is_some()
    }

    /// Eases `value` to `to` and repaints the node `id` on every frame until
    /// it gets there.
    ///
    /// The node's painter reads the value with [`Ui::frame_time`]. Starting a
    /// value that is already moving turns it round from where it is. When
    /// [`Ui::animations`] is `false` the value lands on `to` at once and the
    /// node is repainted once, so a widget needs no second code path. Frames
    /// are paced by the backend to the display's refresh rate, and stop when
    /// nothing is moving.
    ///
    /// The animation ends when `id` is destroyed through [`Ui::destroy`] (so
    /// when the widget owning it is dropped), not when a node above it is. A
    /// value shared by two nodes is animated on each separately.
    pub fn animate(&self, id: WidgetId, value: &Rc<Animated>, to: f32, motion: Motion) {
        self.start(id, value, None, to, motion);
    }

    /// [`Ui::animate`], also calling `step` with the value on every frame,
    /// the last one included (and once, with `to`, when the change is
    /// instant). For an animation that has to move a node, not only repaint
    /// it: a popup unrolling.
    pub fn animate_with(
        &self,
        id: WidgetId,
        value: &Rc<Animated>,
        to: f32,
        motion: Motion,
        step: impl Fn(f32) + 'static,
    ) {
        self.start(id, value, Some(Box::new(step)), to, motion);
    }

    /// The time of the frame being painted, on the backend's clock: what a
    /// painter passes to [`Animated::value`]. It is latched once per frame, so
    /// every painter of a frame sees the same instant.
    pub fn frame_time(&self) -> Duration {
        self.core.motion().frame_time().get()
    }

    /// A shared handle to [`Ui::frame_time`], so a widget's painter reads it
    /// live without keeping the window alive.
    pub fn frame_time_handle(&self) -> Rc<Cell<Duration>> {
        self.core.motion().frame_time()
    }

    fn start(
        &self,
        id: WidgetId,
        value: &Rc<Animated>,
        step: Option<crate::app::motion::Step>,
        to: f32,
        motion: Motion,
    ) {
        let animate = !self.is_design_mode() && !self.prefers_reduced_motion();
        self.core.motion().start(
            self.core.backend().as_ref(),
            self.core.window(),
            animate,
            (id, value, step),
            to,
            motion,
        );
    }
}
