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

impl Entry {
    fn is(&self, id: WidgetId, value: &Rc<Animated>) -> bool {
        self.id == id && Rc::ptr_eq(&self.value, value)
    }
}

/// What was removed while a frame had the entries out of [`Driver::active`],
/// where a removal cannot reach them.
enum Removed {
    /// Everything animating on a node, which was destroyed.
    Node(WidgetId),
    /// One value on one node, which was started again.
    Value(WidgetId, Rc<Animated>),
}

impl Removed {
    fn covers(&self, entry: &Entry) -> bool {
        match self {
            Removed::Node(id) => entry.id == *id,
            Removed::Value(id, value) => entry.is(*id, value),
        }
    }
}

/// A window's animations.
pub(crate) struct Driver {
    /// The application's switch ([`Ui::set_animations`](super::Ui)).
    enabled: Cell<bool>,
    /// The time of the frame being painted, shared with the painters.
    frame_time: Rc<Cell<Duration>>,
    active: RefCell<Vec<Entry>>,
    /// Set while [`Driver::frame`] runs the entries, with what was removed
    /// meanwhile.
    in_frame: Cell<bool>,
    removed: RefCell<Vec<Removed>>,
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
            in_frame: Cell::new(false),
            removed: RefCell::new(Vec::new()),
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
        self.remove(Removed::Value(id, Rc::clone(value)));
        let now = backend.now().filter(|_| animate && self.enabled.get());
        let Some(now) = now.filter(|_| self.schedule(backend, window)) else {
            value.jump(to);
            if let Some(step) = &step {
                step(to);
            }
            backend.invalidate(id);
            return;
        };
        value.retarget(to, now, motion);
        self.active.borrow_mut().push(Entry {
            id,
            value: Rc::clone(value),
            step,
        });
        backend.invalidate(id);
    }

    /// Drops what is animating on `id`, when its node is destroyed.
    pub(crate) fn cancel(&self, id: WidgetId) {
        self.remove(Removed::Node(id));
    }

    /// Drops every animation, when the window goes: a step may hold the
    /// window's `Ui`, which would keep the window alive.
    pub(crate) fn clear(&self) {
        let entries = self.active.take();
        // Dropped outside the borrow: a step may own a widget, whose drop
        // cancels its animations here.
        drop(entries);
    }

    /// Advances one frame: latches the time, steps and repaints what is
    /// moving, and asks for the next frame while anything still is.
    pub(crate) fn frame(&self, backend: &dyn Backend, window: WindowId) {
        self.requested.set(false);
        // A step may start or cancel an animation, so the entries are run
        // outside the borrow, and what is removed meanwhile is noted.
        let mut entries = self.active.take();
        self.in_frame.set(true);
        match backend.now() {
            Some(now) => {
                self.frame_time.set(now);
                entries.retain(|entry| {
                    if self.was_removed(entry) {
                        return false;
                    }
                    if let Some(step) = &entry.step {
                        step(entry.value.value(now));
                    }
                    backend.invalidate(entry.id);
                    // The frame that reaches the target is the entry's last.
                    entry.value.is_running(now)
                });
            }
            // The backend lost its clock: nothing can move any more, so
            // everything lands where it was going.
            None => entries.retain(|entry| {
                if !self.was_removed(entry) {
                    let to = entry.value.target();
                    entry.value.jump(to);
                    if let Some(step) = &entry.step {
                        step(to);
                    }
                    backend.invalidate(entry.id);
                }
                false
            }),
        }
        self.in_frame.set(false);
        // A step that ran early in the frame may have removed an entry that
        // ran before it.
        entries.retain(|entry| !self.was_removed(entry));
        self.removed.take();
        // What the steps started comes after what was already moving.
        let mut started = self.active.take();
        entries.append(&mut started);
        let idle = entries.is_empty();
        self.active.replace(entries);
        if !idle {
            self.schedule(backend, window);
        } else if let Some(timer) = self.fallback.take() {
            backend.kill_timer(window, timer);
        }
    }

    /// Whether `entry` was removed since the running frame took it.
    fn was_removed(&self, entry: &Entry) -> bool {
        self.removed
            .borrow()
            .iter()
            .any(|removed| removed.covers(entry))
    }

    /// Removes the entries `what` covers. They are dropped outside the borrow:
    /// a step may own a widget, whose drop comes back here.
    fn remove(&self, what: Removed) {
        let mut dropped = Vec::new();
        {
            let mut active = self.active.borrow_mut();
            let mut at = 0;
            while at < active.len() {
                if what.covers(&active[at]) {
                    dropped.push(active.remove(at));
                } else {
                    at += 1;
                }
            }
        }
        drop(dropped);
        if self.in_frame.get() {
            self.removed.borrow_mut().push(what);
        }
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
