#![forbid(unsafe_code)]

//! The runtime that owns an app and drains its message queue, the [`Host`]
//! trait the backend's event sink drives, and [`run_app`].

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use super::{App, Core, TimerListener, Ui};
use crate::backend::{Backend, Event, PlatformSpec, Result, WidgetId, WindowId};
use crate::router::WidgetHost;

/// The runtime that owns the app and drains the queue into it.
pub(crate) struct Runtime<A: App> {
    core: Rc<Core<A::Msg>>,
    app: RefCell<Option<A>>,
    quits_on_close: bool,
    /// Set once the window this runtime drives has closed, so the sink can drop
    /// the runtime from the secondary-window registry after the event returns.
    closed: Cell<bool>,
}

/// The part of a runtime a backend's event sink needs, erased over the app
/// type so one sink and one registry serve every window.
pub(crate) trait Host {
    /// The window this host drives.
    fn window(&self) -> WindowId;
    /// Offers `event` to the host's app.
    fn deliver(&self, target: WidgetId, event: &Event) -> bool;
    /// Whether the window has closed.
    fn is_closed(&self) -> bool;
    /// Records that the window is closing.
    fn mark_closed(&self);
}

impl<A: App> Runtime<A> {
    /// A runtime for a window whose close quits the loop.
    pub(crate) fn primary(core: Rc<Core<A::Msg>>, app: A) -> Rc<Runtime<A>> {
        Runtime::new(core, app, true)
    }

    /// A runtime over `core`, owning `app`. `quits_on_close` is whether closing
    /// the window also ends the loop (false for a secondary window).
    pub(crate) fn new(core: Rc<Core<A::Msg>>, app: A, quits_on_close: bool) -> Rc<Runtime<A>> {
        Rc::new(Runtime {
            core,
            app: RefCell::new(Some(app)),
            quits_on_close,
            closed: Cell::new(false),
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
                    // Clone the listeners out first: one may register or remove
                    // another timer listener, and running it under the borrow
                    // would fight the borrow flag.
                    let listeners: Vec<TimerListener<A::Msg>> = self
                        .core
                        .timer_listeners
                        .borrow()
                        .iter()
                        .map(|(_, listener)| Rc::clone(listener))
                        .collect();
                    let listened: Vec<A::Msg> = listeners
                        .iter()
                        .filter_map(|listener| listener(*id))
                        .collect();
                    for msg in listened {
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
                Event::DpiChanged { dpi, suggested } => {
                    let mapped = self
                        .core
                        .on_dpi_changed
                        .borrow()
                        .as_ref()
                        .and_then(|f| f(*dpi, *suggested));
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
            // The app intercepted the close and keeps the window open.
            Some(msg) => self.core.enqueue(msg),
            None => {
                self.closed.set(true);
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
    pub(crate) fn drain(&self) {
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

impl<A: App> Host for Runtime<A> {
    fn window(&self) -> WindowId {
        self.core.window()
    }

    fn deliver(&self, target: WidgetId, event: &Event) -> bool {
        Runtime::deliver(self, target, event)
    }

    fn is_closed(&self) -> bool {
        self.closed.get()
    }

    fn mark_closed(&self) {
        self.closed.set(true);
    }
}

impl<A: App> Drop for Runtime<A> {
    fn drop(&mut self) {
        // Idempotent: clears this window's registry entry if the runtime is
        // dropped without a close having done it (for example at process exit).
        super::secondary::unregister_window(self.core.window());
    }
}

/// The sink a backend drives: a weak handle to a runtime, so the runtime (and
/// its backend) can be dropped without a reference cycle. After every event it
/// drops a window that has closed from the secondary-window registry.
pub(crate) struct Sink {
    host: Weak<dyn Host>,
}

impl Sink {
    /// A sink for `host`, held weakly.
    pub(crate) fn new(host: Weak<dyn Host>) -> Sink {
        Sink { host }
    }
}

impl WidgetHost for Sink {
    fn deliver(&self, target: WidgetId, event: &Event) -> bool {
        let Some(host) = self.host.upgrade() else {
            return false;
        };
        let handled = host.deliver(target, event);
        if host.is_closed() {
            super::secondary::unregister_window(host.window());
        }
        handled
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
    let host: Rc<dyn Host> = Rc::clone(&runtime) as Rc<dyn Host>;
    backend.set_event_sink(window, Rc::new(Sink::new(Rc::downgrade(&host))));
    // Flush anything `make` queued before the sink existed, so a synchronous
    // backend's `run` returning immediately cannot strand it.
    runtime.drain();
    backend.run();
    backend.close_window(window);
    Ok(())
}
