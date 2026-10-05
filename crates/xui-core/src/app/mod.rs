#![forbid(unsafe_code)]

//! The portable application runtime: [`App`], the per-window [`Core`] message
//! queue, the [`Ui`] handle widgets use, and [`run_app`].
//!
//! A backend decodes native input into [`Event`](crate::backend::Event)s and
//! delivers them to a [`WidgetHost`](crate::router::WidgetHost) sink installed
//! on the window. Widget event mappers turn an event into the app's `Msg`; the
//! queue wakes the backend, and the runtime drains it into [`App::update`] —
//! which is never re-entered, because the app is borrowed for the whole call and
//! a drain that runs while it is borrowed puts its message back.

mod design;
mod launch;
mod proxy;
mod runtime;
mod secondary;
#[cfg(test)]
mod tests;
mod ui;

pub use launch::{Launch, app};
pub use proxy::Proxy;
pub use runtime::run_app;
pub use secondary::WindowHandle;
pub use ui::Ui;

// `Runtime` is used by the widget unit tests; the `pub(crate)` re-export is
// otherwise unused in a non-test build.
#[allow(unused_imports)]
pub(crate) use runtime::Runtime;

use std::any::Any;
use std::cell::{Cell, RefCell};
use std::collections::{HashSet, VecDeque};
use std::rc::Rc;
use std::sync::Arc;

use proxy::Inbox;

use crate::backend::{Backend, TimerId, WidgetId, WindowId};
use crate::geometry::Rect;
use crate::router::Router;
use crate::theme::Theme;

/// A widget-layer application.
///
/// Widget events are mapped to the app's own [`App::Msg`] by closures given
/// when the widgets are built, queued, and delivered to [`App::update`], which
/// is never re-entered.
pub trait App: 'static {
    /// The application's message type.
    type Msg: 'static;

    /// Handles one message. Use `ui` to build widgets, change the window, or
    /// enqueue further messages.
    fn update(&mut self, msg: Self::Msg, ui: &mut Ui<Self::Msg>);
}

/// A close request mapped to an optional app message.
type CloseMapper<M> = Box<dyn Fn() -> Option<M>>;
/// A shortcut key mapped to an optional app message.
type KeyMapper<M> = Box<dyn Fn(crate::message::Key, crate::message::Modifiers) -> Option<M>>;
/// A timer tick mapped to an optional app message.
type TimerMapper<M> = Box<dyn Fn(TimerId) -> Option<M>>;
/// A widget's own timer listener, shared so one may be cloned out to run.
type TimerListener<M> = Rc<dyn Fn(TimerId) -> Option<M>>;
/// A display-layout change mapped to an optional app message.
type DisplayMapper<M> = Box<dyn Fn() -> Option<M>>;
/// A DPI change mapped to an optional app message, with the new dots-per-inch
/// and the backend's suggested window bounds in device pixels.
type DpiMapper<M> = Box<dyn Fn(u32, Rect) -> Option<M>>;
/// The layout passes [`Core::flush_layout`] runs before deferring the rest.
const LAYOUT_PASSES: usize = 4;
/// The deferred layout passes it schedules in a row before giving up.
const LAYOUT_RETRIES: u8 = 3;

/// A mounted layout, as the window it lives in sees it.
pub(crate) trait LayoutHook {
    /// Lays the layout out again (a layout its container drives does
    /// nothing: the container re-lays it when it is placed).
    fn relayout(&self);
    /// Appends the layout's tree, rectangles and warnings to `out`.
    fn report(&self, out: &mut String);
    /// Appends the rectangle of every node a window-level layout placed, in
    /// the window's client coordinates; a layout inside a container adds none.
    fn rects(&self, out: &mut Vec<Rect>);
}

/// The shared, interior-mutable state behind a window's [`Ui`].
pub(crate) struct Core<M> {
    backend: Rc<dyn Backend>,
    window: WindowId,
    queue: RefCell<VecDeque<M>>,
    inbox: Arc<Inbox<M>>,
    theme: Rc<Cell<Theme>>,
    /// The window's root design-mode scope (a form designer): widgets under it
    /// ignore their own input so the editor can select and move them.
    design_root: Rc<design::DesignScope>,
    router: Router,
    on_close: RefCell<Option<CloseMapper<M>>>,
    on_key: RefCell<Option<KeyMapper<M>>>,
    on_timer: RefCell<Option<TimerMapper<M>>>,
    /// Per-widget timer listeners, told apart from the app's mapping so a
    /// widget can watch its own timer without displacing [`Ui::on_timer`].
    timer_listeners: RefCell<Vec<(usize, TimerListener<M>)>>,
    next_timer_listener: Cell<usize>,
    on_display_change: RefCell<Option<DisplayMapper<M>>>,
    on_dpi_changed: RefCell<Option<DpiMapper<M>>>,
    /// Nodes hidden through [`Ui::set_visible`], which layout leaves out.
    hidden: RefCell<HashSet<u64>>,
    /// Mounted layouts, re-laid when the window resizes or its DPI changes,
    /// and once per delivered event after something marked them dirty.
    layout_hooks: RefCell<Vec<(usize, Rc<dyn LayoutHook>)>>,
    next_layout_hook: Cell<usize>,
    /// Set by a change that may alter a widget's natural size (its text, its
    /// visibility, the theme); [`Core::flush_layout`] clears it.
    layout_dirty: Cell<bool>,
    /// Deferred layout passes scheduled in a row by [`Core::flush_layout`]
    /// for layouts that would not settle; reset once they do.
    layout_retries: Cell<u8>,
    /// A value a modal child closes with (see [`Ui::close_with_result`]). The
    /// opener reads it after the child's loop returns; `Any` erases its type
    /// until then.
    result: RefCell<Option<Box<dyn Any>>>,
    /// Values kept alive as long as the window: the layout [`Ui::root`]
    /// mounted.
    retained: RefCell<Vec<Box<dyn Any>>>,
}

impl<M> Core<M> {
    /// A core for `window`, driven by `backend`.
    pub(crate) fn new(backend: Rc<dyn Backend>, window: WindowId) -> Rc<Core<M>> {
        Rc::new(Core {
            backend,
            window,
            queue: RefCell::new(VecDeque::new()),
            inbox: Inbox::new(),
            theme: Rc::new(Cell::new(Theme::light())),
            design_root: design::DesignScope::root(),
            router: Router::new(),
            on_close: RefCell::new(None),
            on_key: RefCell::new(None),
            on_timer: RefCell::new(None),
            timer_listeners: RefCell::new(Vec::new()),
            next_timer_listener: Cell::new(0),
            on_display_change: RefCell::new(None),
            on_dpi_changed: RefCell::new(None),
            hidden: RefCell::new(HashSet::new()),
            layout_hooks: RefCell::new(Vec::new()),
            next_layout_hook: Cell::new(0),
            layout_dirty: Cell::new(false),
            layout_retries: Cell::new(0),
            result: RefCell::new(None),
            retained: RefCell::new(Vec::new()),
        })
    }

    /// The window this core drives.
    pub(crate) fn window(&self) -> WindowId {
        self.window
    }

    /// The backend this core drives.
    pub(crate) fn backend(&self) -> Rc<dyn Backend> {
        Rc::clone(&self.backend)
    }

    /// The node event router.
    pub(crate) fn router(&self) -> &Router {
        &self.router
    }

    /// The window's current theme, shared with the widgets' painters.
    pub(crate) fn theme(&self) -> Rc<Cell<Theme>> {
        Rc::clone(&self.theme)
    }

    /// The window's root design-mode scope.
    pub(crate) fn design_root(&self) -> Rc<design::DesignScope> {
        Rc::clone(&self.design_root)
    }

    /// Appends `msg` and wakes the backend on the empty-to-non-empty edge, so a
    /// burst of messages costs a single drain.
    pub(crate) fn enqueue(&self, msg: M) {
        let was_empty = self.queue.borrow().is_empty();
        self.queue.borrow_mut().push_back(msg);
        if was_empty {
            self.backend.wake(self.window);
        }
    }

    /// A proxy for sending messages from worker threads.
    pub(crate) fn proxy(&self) -> Proxy<M>
    where
        M: Send,
    {
        Proxy::new(Arc::clone(&self.inbox), self.backend.waker(self.window))
    }

    /// Moves every worker-thread message into the queue, ahead of the drain.
    ///
    /// `enqueue` may post one extra wake while handling the current one; it
    /// finds an empty inbox and stops, so this cannot become a wake storm.
    fn collect_inbox(&self) {
        for msg in self.inbox.collect() {
            self.enqueue(msg);
        }
    }

    /// Records the close mapper.
    pub(crate) fn set_on_close(&self, f: impl Fn() -> Option<M> + 'static) {
        self.on_close.replace(Some(Box::new(f)));
    }

    /// Records the shortcut-key mapper.
    pub(crate) fn set_on_key(
        &self,
        f: impl Fn(crate::message::Key, crate::message::Modifiers) -> Option<M> + 'static,
    ) {
        self.on_key.replace(Some(Box::new(f)));
    }

    /// Records the timer mapper.
    pub(crate) fn set_on_timer(&self, f: impl Fn(TimerId) -> Option<M> + 'static) {
        self.on_timer.replace(Some(Box::new(f)));
    }

    /// Adds a widget's timer listener, returning a token to remove it again.
    pub(crate) fn add_timer_listener(&self, f: impl Fn(TimerId) -> Option<M> + 'static) -> usize {
        let token = self.next_timer_listener.get();
        self.next_timer_listener.set(token.wrapping_add(1));
        self.timer_listeners.borrow_mut().push((token, Rc::new(f)));
        token
    }

    /// Removes the listener `token` returned by [`Core::add_timer_listener`].
    pub(crate) fn remove_timer_listener(&self, token: usize) {
        self.timer_listeners
            .borrow_mut()
            .retain(|(existing, _)| *existing != token);
    }

    /// Records the display-change mapper.
    pub(crate) fn set_on_display_change(&self, f: impl Fn() -> Option<M> + 'static) {
        self.on_display_change.replace(Some(Box::new(f)));
    }

    /// Records the DPI-change mapper.
    pub(crate) fn set_on_dpi_changed(&self, f: impl Fn(u32, Rect) -> Option<M> + 'static) {
        self.on_dpi_changed.replace(Some(Box::new(f)));
    }

    /// Records whether `id` is hidden, returning whether that changed it.
    pub(crate) fn set_hidden(&self, id: WidgetId, hidden: bool) -> bool {
        let mut set = self.hidden.borrow_mut();
        if hidden {
            set.insert(id.raw())
        } else {
            set.remove(&id.raw())
        }
    }

    /// Whether `id` was hidden through [`Ui::set_visible`].
    pub(crate) fn is_hidden(&self, id: WidgetId) -> bool {
        self.hidden.borrow().contains(&id.raw())
    }

    /// Adds a mounted layout, returning a token to remove it again.
    pub(crate) fn add_layout_hook(&self, hook: Rc<dyn LayoutHook>) -> usize {
        let token = self.next_layout_hook.get();
        self.next_layout_hook.set(token.wrapping_add(1));
        self.layout_hooks.borrow_mut().push((token, hook));
        token
    }

    /// Removes the callback `token` returned by [`Core::add_layout_hook`].
    pub(crate) fn remove_layout_hook(&self, token: usize) {
        self.layout_hooks
            .borrow_mut()
            .retain(|(existing, _)| *existing != token);
    }

    /// The mounted layouts, cloned out of the borrow: one may mount or drop
    /// another layout, which would fight the borrow flag.
    pub(crate) fn layout_hooks(&self) -> Vec<Rc<dyn LayoutHook>> {
        self.layout_hooks
            .borrow()
            .iter()
            .map(|(_, hook)| Rc::clone(hook))
            .collect()
    }

    /// Lays every mounted layout out again now.
    pub(crate) fn run_layout_hooks(&self) {
        self.layout_dirty.set(false);
        for hook in self.layout_hooks() {
            hook.relayout();
        }
    }

    /// Marks the layouts as needing a pass, run by [`Core::flush_layout`].
    pub(crate) fn invalidate_layout(&self) {
        self.layout_dirty.set(true);
    }

    /// Runs the layout pass a change asked for, if any. A pass may itself
    /// change visibility (a scroll bar appearing), so it repeats while that
    /// happens, a few times at most. A layout still dirty after that (a
    /// widget whose `placed` keeps changing its own size) gets one deferred
    /// pass through a wake rather than being left stale until the next
    /// input, up to [`LAYOUT_RETRIES`] in a row so it cannot spin forever.
    pub(crate) fn flush_layout(&self) {
        for _ in 0..LAYOUT_PASSES {
            if !self.layout_dirty.get() {
                self.layout_retries.set(0);
                return;
            }
            self.run_layout_hooks();
        }
        if !self.layout_dirty.get() {
            self.layout_retries.set(0);
        } else if self.layout_retries.get() < LAYOUT_RETRIES {
            self.layout_retries.set(self.layout_retries.get() + 1);
            self.backend.wake(self.window);
        }
    }

    /// Keeps `value` alive until [`Core::release_retained`].
    pub(crate) fn retain(&self, value: Box<dyn Any>) {
        self.retained.borrow_mut().push(value);
    }

    /// Drops what [`Core::retain`] kept, outside the borrow: a retained layout
    /// removes its hooks from this core as it drops.
    pub(crate) fn release_retained(&self) {
        let retained = self.retained.take();
        drop(retained);
    }

    /// Stores the value a modal child closes with.
    pub(crate) fn set_result(&self, value: Box<dyn Any>) {
        self.result.replace(Some(value));
    }

    /// Takes the value a modal child closed with, downcast to `R`.
    pub(crate) fn take_result<R: 'static>(&self) -> Option<R> {
        self.result
            .borrow_mut()
            .take()
            .and_then(|value| value.downcast::<R>().ok())
            .map(|value| *value)
    }

    fn pop(&self) -> Option<M> {
        self.queue.borrow_mut().pop_front()
    }

    fn put_back(&self, msg: M) {
        self.queue.borrow_mut().push_front(msg);
    }
}

impl<M> Drop for Core<M> {
    fn drop(&mut self) {
        // A worker may still hold a `Proxy`; closing the inbox makes its later
        // sends fail instead of queueing messages nobody will drain.
        self.inbox.close();
    }
}
