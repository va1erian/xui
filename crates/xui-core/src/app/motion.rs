#![forbid(unsafe_code)]

//! The animation driver of one window: which [`Animated`] values are moving,
//! and the frames that move them.
//!
//! It owns no clock and no loop. [`Driver::start`] records a value and asks
//! the backend for a frame; each frame latches the backend's time, repaints
//! the nodes that are animating and asks for the next one while any remain,
//! so an idle window is never ticked.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use crate::backend::{Backend, TimerId, WidgetId, WindowId};
use crate::motion::{Animated, Motion};

/// The period of the fallback timer, for a backend with no frame pacing: one
/// 60 Hz frame, rounded down to whole milliseconds.
const FALLBACK_MILLIS: u32 = 16;

/// Runs each frame with the value's current reading, for the few animations
/// that have to move a node rather than only repaint it.
pub(crate) type Step = Box<dyn Fn(f32)>;

/// One value in flight, and the node to repaint while it moves.
struct Entry {
    id: WidgetId,
    value: Rc<Animated>,
    step: Option<Step>,
}

/// A window's animations.
pub(crate) struct Driver {
    /// The application's switch ([`Ui::set_animations`](super::Ui)).
    enabled: Cell<bool>,
    /// The time of the frame being painted, shared with the painters.
    frame_time: Rc<Cell<Duration>>,
    active: RefCell<Vec<Entry>>,
    /// A frame was requested from the backend and has not arrived.
    requested: Cell<bool>,
    /// The timer ticking the frames on a backend with no frame pacing.
    fallback: Cell<Option<TimerId>>,
}

impl Driver {
    pub(crate) fn new() -> Driver {
        Driver {
            enabled: Cell::new(true),
            frame_time: Rc::new(Cell::new(Duration::ZERO)),
            active: RefCell::new(Vec::new()),
            requested: Cell::new(false),
            fallback: Cell::new(None),
        }
    }

    /// Turns animations on or off for the window.
    pub(crate) fn set_enabled(&self, on: bool) {
        self.enabled.set(on);
    }

    /// Whether the application left animations on.
    pub(crate) fn enabled(&self) -> bool {
        self.enabled.get()
    }

    /// The time of the frame being painted.
    pub(crate) fn frame_time(&self) -> Rc<Cell<Duration>> {
        Rc::clone(&self.frame_time)
    }

    /// Whether `timer` is this driver's own fallback tick.
    pub(crate) fn owns_timer(&self, timer: TimerId) -> bool {
        self.fallback.get() == Some(timer)
    }

    /// Starts `value` moving to `to` and schedules its frames; `id` is
    /// repainted on each. With `animate` false, or on a backend with no clock
    /// or no way to tick, the value lands at once instead.
    pub(crate) fn start(
        &self,
        backend: &dyn Backend,
        window: WindowId,
        animate: bool,
        entry: (WidgetId, &Rc<Animated>, Option<Step>),
        to: f32,
        motion: Motion,
    ) {
        let (id, value, step) = entry;
        let now = backend.now().filter(|_| animate && self.enabled.get());
        let Some(now) = now.filter(|_| self.schedule(backend, window)) else {
            value.jump(to);
            self.forget(value);
            if let Some(step) = &step {
                step(to);
            }
            backend.invalidate(id);
            return;
        };
        value.retarget(to, now, motion);
        self.forget(value);
        self.active.borrow_mut().push(Entry {
            id,
            value: Rc::clone(value),
            step,
        });
        backend.invalidate(id);
    }

    /// Drops what is animating on `id`, when its node is destroyed.
    pub(crate) fn cancel(&self, id: WidgetId) {
        self.active.borrow_mut().retain(|entry| entry.id != id);
    }

    /// Advances one frame: latches the time, steps and repaints what is
    /// moving, and asks for the next frame while anything still is.
    pub(crate) fn frame(&self, backend: &dyn Backend, window: WindowId) {
        self.requested.set(false);
        let Some(now) = backend.now() else {
            return;
        };
        self.frame_time.set(now);
        // A step may start another animation, so run them outside the borrow.
        let mut entries = self.active.take();
        entries.retain(|entry| {
            if let Some(step) = &entry.step {
                step(entry.value.value(now));
            }
            backend.invalidate(entry.id);
            // The frame that reaches the target is the entry's last.
            entry.value.is_running(now)
        });
        {
            let mut active = self.active.borrow_mut();
            // What a step started replaces an older entry for the same value.
            entries.retain(|old| !active.iter().any(|new| Rc::ptr_eq(&new.value, &old.value)));
            entries.append(&mut active);
            *active = entries;
        }
        if self.active.borrow().is_empty() {
            if let Some(timer) = self.fallback.take() {
                backend.kill_timer(window, timer);
            }
        } else {
            self.schedule(backend, window);
        }
    }

    /// Removes the entry animating `value`, if any.
    fn forget(&self, value: &Rc<Animated>) {
        self.active
            .borrow_mut()
            .retain(|entry| !Rc::ptr_eq(&entry.value, value));
    }

    /// Makes sure a frame is coming; `false` when the backend can neither
    /// pace frames nor start a timer.
    fn schedule(&self, backend: &dyn Backend, window: WindowId) -> bool {
        if self.requested.get() || self.fallback.get().is_some() {
            return true;
        }
        if backend.request_frame(window) {
            self.requested.set(true);
            return true;
        }
        let timer = backend.set_timer(window, FALLBACK_MILLIS);
        if timer.0 == 0 {
            return false;
        }
        self.fallback.set(Some(timer));
        true
    }
}
