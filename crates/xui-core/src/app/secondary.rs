#![forbid(unsafe_code)]

//! Secondary top-level windows: non-modal [`Ui::open_window`] and modal
//! [`Ui::open_modal`], plus the [`WindowHandle`] the opener keeps.
//!
//! Every window opened this way runs its own [`App`] with its own `Msg` type and
//! its own message queue, so its `update` is never re-entered (the same
//! invariant as the main window), while all windows share the one backend event
//! loop. A non-modal window is independent of the opener but does not quit the
//! loop when it closes; a modal window disables the opener and blocks on
//! [`Backend::run_modal`](crate::backend::Backend::run_modal) until it closes,
//! then returns the value the child set with [`Ui::close_with_result`].
//!
//! The runtime for each secondary window is kept alive in a thread-local
//! registry (the backend holds only a weak sink), and dropped when the window
//! closes.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::{Rc, Weak};

use super::runtime::{Host, Runtime, Sink};
use super::{App, Core, Ui};
use crate::backend::{Backend, NativeWindowHandle, PlatformSpec, Result, WindowId};
use crate::image::Image;
use crate::theme::Theme;

thread_local! {
    /// Every open secondary window's runtime, keyed by its window.
    static HOSTS: RefCell<HashMap<u64, Rc<dyn Host>>> = RefCell::new(HashMap::new());
}

/// Forgets `window`'s runtime, dropping the app it owns.
///
/// Idempotent, and safe to call from a runtime's own drop: the removed value is
/// dropped *after* the registry borrow is released. `try_with` keeps a drop that
/// happens during thread-local teardown from panicking.
pub(crate) fn unregister_window(window: WindowId) {
    let _ = HOSTS.try_with(|hosts| {
        let removed = hosts.borrow_mut().remove(&window.raw());
        drop(removed);
    });
}

/// Marks `window`'s runtime closed, so its sink unregisters it after the event
/// in flight returns. A no-op for the primary window, which has no registry
/// entry.
pub(crate) fn mark_closed(window: WindowId) {
    let _ = HOSTS.try_with(|hosts| {
        if let Some(host) = hosts.borrow().get(&window.raw()) {
            host.mark_closed();
        }
    });
}

/// A handle to a secondary window opened with [`Ui::open_window`], held by the
/// opener. It sends messages to the child's queue and can retitle, capture or
/// close it.
pub struct WindowHandle<M> {
    core: Rc<Core<M>>,
    backend: Rc<dyn Backend>,
    window: WindowId,
}

impl<M> Clone for WindowHandle<M> {
    fn clone(&self) -> WindowHandle<M> {
        WindowHandle {
            core: Rc::clone(&self.core),
            backend: Rc::clone(&self.backend),
            window: self.window,
        }
    }
}

impl<M: 'static> WindowHandle<M> {
    /// The window this handle drives.
    pub fn window(&self) -> WindowId {
        self.window
    }

    /// Sends `msg` to the child app's [`update`](App::update). A message sent
    /// after the window has closed is queued but never delivered.
    pub fn send(&self, msg: M) {
        self.core.enqueue(msg);
    }

    /// Whether the window is still open.
    pub fn is_open(&self) -> bool {
        HOSTS.with(|hosts| hosts.borrow().contains_key(&self.window.raw()))
    }

    /// Closes the window. Safe to call more than once.
    pub fn close(&self) {
        self.backend.close_window(self.window);
        unregister_window(self.window);
    }

    /// Replaces the window's title.
    pub fn set_title(&self, title: &str) {
        self.backend.set_window_title(self.window, title);
    }

    /// The backend's native handle for the window, for an OS integration.
    pub fn native(&self) -> Option<NativeWindowHandle> {
        self.backend.native_window(self.window)
    }

    /// Renders the window's current content into an image.
    pub fn capture(&self) -> Result<Image> {
        self.backend.capture(self.window)
    }
}

/// Opens a non-modal secondary window running its own app.
pub(crate) fn open_secondary<B, F>(
    backend: Rc<dyn Backend>,
    theme: Theme,
    spec: &PlatformSpec,
    make: F,
) -> Result<WindowHandle<B::Msg>>
where
    B: App + 'static,
    F: FnOnce(&mut Ui<B::Msg>) -> B,
{
    let built = build(backend, theme, spec, make)?;
    built.runtime.drain();
    // A child that closed itself while its app was being built has no later
    // event to reap it.
    if built.runtime.is_closed() {
        unregister_window(built.window);
    }
    let backend = built.core.backend();
    Ok(WindowHandle {
        core: built.core,
        backend,
        window: built.window,
    })
}

/// Opens a modal secondary window, disabling `owner` and blocking until the
/// child closes. Returns the value the child set with
/// [`Ui::close_with_result`], or `None` on a backend that cannot run a modal
/// loop or a child that closed without a result.
pub(crate) fn open_modal<B, F, R>(
    backend: Rc<dyn Backend>,
    owner: WindowId,
    theme: Theme,
    spec: &PlatformSpec,
    make: F,
) -> Option<R>
where
    B: App + 'static,
    F: FnOnce(&mut Ui<B::Msg>) -> B,
    R: 'static,
{
    let built = build(Rc::clone(&backend), theme, spec, make).ok()?;
    built.runtime.drain();

    backend.set_window_enabled(owner, false);
    let ran = backend.run_modal(built.window).is_ok();
    backend.set_window_enabled(owner, true);
    if !ran {
        backend.close_window(built.window);
    }
    let result = built.core.take_result::<R>();
    unregister_window(built.window);
    result
}

/// A built secondary window: its window, core and runtime.
struct Built<B: App> {
    window: WindowId,
    core: Rc<Core<B::Msg>>,
    runtime: Rc<Runtime<B>>,
}

/// Creates the window, its [`Core`], its app and its runtime, installs the
/// event sink and registers the runtime.
fn build<B, F>(
    backend: Rc<dyn Backend>,
    theme: Theme,
    spec: &PlatformSpec,
    make: F,
) -> Result<Built<B>>
where
    B: App + 'static,
    F: FnOnce(&mut Ui<B::Msg>) -> B,
{
    let window = backend.open_window(spec)?;
    let core = Core::new(Rc::clone(&backend), window);
    core.theme().set(theme);
    backend.set_theme(window, &theme);
    let mut ui = Ui::new(Rc::clone(&core));
    let app = make(&mut ui);
    let runtime = Runtime::new(Rc::clone(&core), app, false);
    let host: Rc<dyn Host> = Rc::clone(&runtime) as Rc<dyn Host>;
    backend.set_event_sink(
        window,
        Rc::new(Sink::new(Rc::downgrade(&host) as Weak<dyn Host>)),
    );
    HOSTS.with(|hosts| hosts.borrow_mut().insert(window.raw(), host));
    Ok(Built {
        window,
        core,
        runtime,
    })
}
