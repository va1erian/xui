#![forbid(unsafe_code)]

//! What the widget, its painter, its event mapper and its timer share, and
//! the background layout timer.

use std::cell::{Cell, RefCell};
use std::rc::Weak;
use std::time::Duration;

use xui_core::app::Ui;
use xui_core::backend::{TimerId, WidgetId};
use xui_core::widget::Control;

use super::events::Out;
use super::state::State;

/// How often the timer lays out more of a long document, in milliseconds.
const TICK_MS: u32 = 30;
/// How long one tick may spend laying out.
const SLICE: Duration = Duration::from_millis(4);

pub(super) type LinkMapper<M> = Box<dyn Fn(&str) -> Option<M>>;

/// What the widget, its painter, its event mapper and its timer share. The
/// callbacks hold it weakly, so dropping the view drops it and its node.
pub(super) struct Shared<M: 'static> {
    pub(super) control: Control<M>,
    pub(super) me: Weak<Shared<M>>,
    pub(super) state: RefCell<State>,
    pub(super) ui: Ui<M>,
    pub(super) id: WidgetId,
    pub(super) on_link: RefCell<Option<LinkMapper<M>>>,
    /// The timer laying out the rest of the document, while there is any.
    pub(super) timer: Cell<Option<TimerId>>,
}

impl<M: 'static> Shared<M> {
    /// Starts the background layout timer if blocks wait to be laid out. A
    /// backend without timers (the offscreen one) lays out only what it
    /// paints, which is all it shows.
    pub(super) fn kick(&self) {
        if self.timer.get().is_some() || self.state.borrow().layout.done() {
            return;
        }
        let me = self.me.clone();
        let timer = self.control.set_timer(TICK_MS, move || {
            if let Some(shared) = me.upgrade() {
                shared.tick();
            }
            None
        });
        self.timer.set(timer);
    }

    /// Lays out the next slice of the document; stops the timer once all of
    /// it is laid out.
    fn tick(&self) {
        let finished = {
            let mut state = self.state.borrow_mut();
            // With no size yet (never painted, or hidden at zero size) there
            // is nothing to lay out for: stop until a resize or a paint gives
            // one and kicks the timer again.
            if !state.layout.sized() {
                drop(state);
                if let Some(timer) = self.timer.take() {
                    self.control.kill_timer(timer);
                }
                return;
            }
            state.layout.work(SLICE);
            state.layout.done()
        };
        self.ui.invalidate(self.id);
        if finished && let Some(timer) = self.timer.take() {
            self.control.kill_timer(timer);
        }
    }

    /// Acts on a handled event outside the state borrow, and maps a clicked
    /// link to a message. A `#slug` naming a heading scrolls to it instead.
    pub(super) fn deliver(&self, out: Out) -> Option<M> {
        if out.focus {
            self.ui.focus(self.id);
        }
        match out.capture {
            Some(true) => self.ui.set_capture(self.id),
            Some(false) => self.ui.release_capture(),
            None => {}
        }
        if let Some(cursor) = out.cursor {
            self.ui.set_cursor(self.id, cursor);
        }
        let mut invalidate = out.invalidate;
        let mut message = None;
        if let Some(href) = out.link {
            let mut state = self.state.borrow_mut();
            let anchor = state.layout.doc().anchor_of_link(&href);
            match anchor {
                Some(block) => {
                    state.scroll_to_block(block);
                    invalidate = true;
                }
                None => {
                    drop(state);
                    message = self.on_link.borrow().as_ref().and_then(|map| map(&href));
                }
            }
        }
        if invalidate {
            self.ui.invalidate(self.id);
        }
        message
    }
}
