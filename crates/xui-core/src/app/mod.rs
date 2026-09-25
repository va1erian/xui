#![forbid(unsafe_code)]

//! The portable application runtime: [`App`], the per-window [`Core`] message
//! queue, the [`Ui`] handle widgets use, and [`run_app`].
//!
//! A backend decodes native input into [`Event`]s and delivers them to a
//! [`WidgetHost`] sink installed on the window. Widget event mappers turn an
//! event into the app's `Msg`; the queue wakes the backend, and the runtime
//! drains it into [`App::update`] — which is never re-entered, because the app
//! is borrowed for the whole call and a drain that runs while it is borrowed
//! puts its message back.

mod proxy;
#[cfg(test)]
mod tests;
mod ui;

pub use proxy::Proxy;
pub use ui::Ui;

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::{Rc, Weak};
use std::sync::Arc;

use proxy::Inbox;

use crate::backend::{Backend, Event, PlatformSpec, Result, TimerId, WidgetId, WindowId};
use crate::router::{Router, WidgetHost};

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
/// A timer tick mapped to an optional app message.
type TimerMapper<M> = Box<dyn Fn(TimerId) -> Option<M>>;
/// A display-layout change mapped to an optional app message.
type DisplayMapper<M> = Box<dyn Fn() -> Option<M>>;

/// The shared, interior-mutable state behind a window's [`Ui`].
pub(crate) struct Core<M> {
    backend: Rc<dyn Backend>,
    window: WindowId,
    queue: RefCell<VecDeque<M>>,
    inbox: Arc<Inbox<M>>,
    router: Router,
    on_close: RefCell<Option<CloseMapper<M>>>,
    on_timer: RefCell<Option<TimerMapper<M>>>,
    on_display_change: RefCell<Option<DisplayMapper<M>>>,
}

impl<M> Core<M> {
    /// A core for `window`, driven by `backend`.
    pub(crate) fn new(backend: Rc<dyn Backend>, window: WindowId) -> Rc<Core<M>> {
        Rc::new(Core {
            backend,
            window,
            queue: RefCell::new(VecDeque::new()),
            inbox: Inbox::new(),
            router: Router::new(),
            on_close: RefCell::new(None),
            on_timer: RefCell::new(None),
            on_display_change: RefCell::new(None),
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

    /// Records the timer mapper.
    pub(crate) fn set_on_timer(&self, f: impl Fn(TimerId) -> Option<M> + 'static) {
        self.on_timer.replace(Some(Box::new(f)));
    }

    /// Records the display-change mapper.
    pub(crate) fn set_on_display_change(&self, f: impl Fn() -> Option<M> + 'static) {
        self.on_display_change.replace(Some(Box::new(f)));
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

/// The runtime that owns the app and drains the queue into it.
pub(crate) struct Runtime<A: App> {
    core: Rc<Core<A::Msg>>,
    app: RefCell<Option<A>>,
    quits_on_close: bool,
}

impl<A: App> Runtime<A> {
    /// A runtime for a window whose close quits the loop.
    pub(crate) fn primary(core: Rc<Core<A::Msg>>, app: A) -> Rc<Runtime<A>> {
        Runtime::new(core, app, true)
    }

    /// A runtime over `core`, owning `app`. `quits_on_close` is whether closing
    /// the window also ends the loop (false for a secondary window).
    fn new(core: Rc<Core<A::Msg>>, app: A, quits_on_close: bool) -> Rc<Runtime<A>> {
        Rc::new(Runtime {
            core,
            app: RefCell::new(Some(app)),
            quits_on_close,
        })
    }

    /// Offers `event` to the app: window-level events first, then the target's
    /// widget mapper. Returns whether it was handled.
    pub(crate) fn deliver(&self, target: WidgetId, event: &Event) -> bool {
        if target.is_none() {
            match event {
                Event::Wake => {
                    self.core.collect_inbox();
                    self.drain();
                    return true;
                }
                Event::Close => {
                    self.on_close();
                    return true;
                }
                Event::Timer { id } => {
                    let mapped = self.core.on_timer.borrow().as_ref().and_then(|f| f(*id));
                    if let Some(msg) = mapped {
                        self.core.enqueue(msg);
                    }
                    return true;
                }
                Event::DisplayChange { .. } => {
                    let mapped = self
                        .core
                        .on_display_change
                        .borrow()
                        .as_ref()
                        .and_then(|f| f());
                    if let Some(msg) = mapped {
                        self.core.enqueue(msg);
                    }
                    return true;
                }
                _ => {}
            }
        }
        self.core.router.dispatch(target, event)
    }

    fn on_close(&self) {
        let mapped = self.core.on_close.borrow().as_ref().and_then(|f| f());
        match mapped {
            Some(msg) => self.core.enqueue(msg),
            None => {
                self.core.backend.close_window(self.core.window);
                if self.quits_on_close {
                    self.core.backend.quit(0);
                }
            }
        }
    }

    /// Drains queued messages into [`App::update`], one at a time, until the
    /// queue is empty or the app is busy. The app is borrowed for the whole of
    /// each `update`, so this cannot re-enter it.
    fn drain(&self) {
        loop {
            let Some(msg) = self.core.pop() else {
                return;
            };
            let Ok(mut slot) = self.app.try_borrow_mut() else {
                self.core.put_back(msg);
                return;
            };
            let Some(app) = slot.as_mut() else {
                self.core.put_back(msg);
                return;
            };
            let mut ui = Ui::new(Rc::clone(&self.core));
            app.update(msg, &mut ui);
        }
    }
}

/// The sink a backend drives: a weak handle to the runtime, so the runtime (and
/// its backend) can be dropped without a reference cycle.
struct Sink<A: App> {
    runtime: Weak<Runtime<A>>,
}

impl<A: App> WidgetHost for Sink<A> {
    fn deliver(&self, target: WidgetId, event: &Event) -> bool {
        match self.runtime.upgrade() {
            Some(runtime) => runtime.deliver(target, event),
            None => false,
        }
    }
}

/// Builds a window through `backend`, constructs the app with `make`, and runs
/// the event loop until the window closes or [`Ui::quit`] is called.
pub fn run_app<A, F>(backend: Rc<dyn Backend>, spec: PlatformSpec, make: F) -> Result<()>
where
    A: App,
    F: FnOnce(&mut Ui<A::Msg>) -> A,
{
    backend.init();
    let window = backend.open_window(&spec)?;
    let core = Core::new(Rc::clone(&backend), window);
    let mut ui = Ui::new(Rc::clone(&core));
    let app = make(&mut ui);
    let runtime = Runtime::primary(Rc::clone(&core), app);
    backend.set_event_sink(
        window,
        Rc::new(Sink {
            runtime: Rc::downgrade(&runtime),
        }),
    );
    // Flush anything `make` queued before the sink existed, so a synchronous
    // backend's `run` returning immediately cannot strand it.
    runtime.drain();
    backend.run();
    backend.close_window(window);
    Ok(())
}
