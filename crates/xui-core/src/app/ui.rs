#![forbid(unsafe_code)]

//! The [`Ui`] handle: the widget layer's view of the window behind a [`Core`].

use std::rc::Rc;

use super::design::DesignScope;
use super::{App, Core, Proxy, WindowHandle, secondary};
use crate::backend::{Event, NativeWindowHandle, PlatformSpec, Result, TimerId, WidgetId};
use crate::geometry::Rect;
use crate::image::Image;
use crate::theme::Theme;

mod every;
mod node_ops;

/// The widget layer's handle to a top-level window.
///
/// Cheap to clone (an [`Rc`]). Widget constructors take it, create their node
/// through it, and map their node's events to the app's `Msg`. It deliberately
/// exposes only the portable backend operations, so a widget never names a
/// platform type; [`Ui::backend`] is the crate-internal seam for a backend to
/// add its own widgets.
pub struct Ui<M> {
    core: Rc<Core<M>>,
    /// A container node this handle was scoped to, so widgets it creates become
    /// that container's children. `None` parents to the window.
    parent: Option<WidgetId>,
    /// This handle's design-mode scope, chained to its creator's.
    design: Rc<DesignScope>,
}

impl<M: 'static> Ui<M> {
    /// A handle over `core`.
    pub(crate) fn new(core: Rc<Core<M>>) -> Ui<M> {
        let design = core.design_root();
        Ui {
            core,
            parent: None,
            design,
        }
    }

    /// A handle scoped to the container `parent`: widgets created through it
    /// become children of that node, while messages and the window still belong
    /// to the top-level window.
    pub fn with_parent(&self, parent: WidgetId) -> Ui<M> {
        Ui {
            core: Rc::clone(&self.core),
            parent: Some(parent),
            design: DesignScope::child(&self.design),
        }
    }

    /// The window this handle drives.
    pub fn window(&self) -> crate::backend::WindowId {
        self.core.window()
    }

    /// Replaces the title of this handle's own top-level window.
    pub fn set_window_title(&self, title: &str) {
        self.core
            .backend()
            .set_window_title(self.core.window(), title);
    }

    /// Opens a non-modal secondary window that runs its own [`App`].
    ///
    /// The child inherits this window's theme. The returned [`WindowHandle`]
    /// sends messages to the child, retitles, captures and closes it.
    pub fn open_window<B, F>(&self, spec: PlatformSpec, make: F) -> Result<WindowHandle<B::Msg>>
    where
        B: App + 'static,
        F: FnOnce(&mut Ui<B::Msg>) -> B,
    {
        secondary::open_secondary(self.core.backend(), self.core.theme().get(), &spec, make)
    }

    /// Opens a modal secondary window that runs its own [`App`], disabling this
    /// window and blocking until the child closes.
    ///
    /// The child closes with a value through [`Ui::close_with_result`]; `None`
    /// is returned on a backend that cannot run a modal loop, or when the child
    /// closed without a result.
    pub fn open_modal<B, F, R>(&self, spec: PlatformSpec, make: F) -> Option<R>
    where
        B: App + 'static,
        F: FnOnce(&mut Ui<B::Msg>) -> B,
        R: 'static,
    {
        secondary::open_modal(
            self.core.backend(),
            self.core.window(),
            self.core.theme().get(),
            &spec,
            make,
        )
    }

    /// The backend's native handle for this window, for an OS integration that
    /// needs one (COM, the shell). `None` when the backend exposes none.
    pub fn native_window(&self) -> Option<NativeWindowHandle> {
        self.core.backend().native_window(self.core.window())
    }

    /// Renders this window's current content into an image.
    pub fn capture(&self) -> Result<Image> {
        self.core.backend().capture(self.core.window())
    }

    /// Enqueues `msg` for [`App::update`](super::App::update).
    pub fn emit(&self, msg: M) {
        self.core.enqueue(msg);
    }

    /// Returns a thread-safe handle for sending messages from worker threads.
    /// `Proxy` is `Clone`, and `Send + Sync` when the message type is; its
    /// sends join the same queue as [`Ui::emit`].
    pub fn proxy(&self) -> Proxy<M>
    where
        M: Send,
    {
        self.core.proxy()
    }

    /// Registers a widget's event mapper: it returns `Some(msg)` to raise it, or
    /// `None` to ignore the event.
    pub fn register_events(&self, id: WidgetId, mapper: impl Fn(&Event) -> Option<M> + 'static) {
        let weak = Rc::downgrade(&self.core);
        self.core.router().register(id, move |event| {
            let Some(msg) = mapper(event) else {
                return false;
            };
            if let Some(core) = weak.upgrade() {
                core.enqueue(msg);
            }
            true
        });
    }

    /// Removes a widget's event mapper.
    pub fn unregister_events(&self, id: WidgetId) {
        self.core.router().unregister(id);
    }

    /// Adds an event listener to `id` alongside any handler already there,
    /// returning a token that [`Ui::remove_events`] uses to remove it. A
    /// side feature (a tooltip) uses this so it can observe a widget without
    /// displacing the widget's own handler.
    pub(crate) fn add_events(
        &self,
        id: WidgetId,
        mapper: impl Fn(&Event) -> Option<M> + 'static,
    ) -> usize {
        let weak = Rc::downgrade(&self.core);
        self.core.router().add(id, move |event| {
            let Some(msg) = mapper(event) else {
                return false;
            };
            if let Some(core) = weak.upgrade() {
                core.enqueue(msg);
            }
            true
        })
    }

    /// Removes the listener `token` returned by [`Ui::add_events`].
    pub(crate) fn remove_events(&self, id: WidgetId, token: usize) {
        self.core.router().remove(id, token);
    }

    /// The window's dots-per-inch.
    pub fn dpi(&self) -> u32 {
        self.core.backend().dpi(self.core.window())
    }

    /// The window's client area, in device pixels.
    pub fn client_rect(&self) -> Rect {
        self.core.backend().client_rect(self.core.window())
    }

    /// The window's current theme.
    pub fn theme(&self) -> Theme {
        self.core.theme().get()
    }

    /// A shared handle to the window's theme, so a widget's painter reads it
    /// live without keeping the window alive.
    pub fn theme_handle(&self) -> Rc<std::cell::Cell<Theme>> {
        self.core.theme()
    }

    /// Applies `theme` to the window and its nodes.
    pub fn set_theme(&self, theme: Theme) {
        self.core.theme().set(theme);
        self.core.backend().set_theme(self.core.window(), &theme);
    }

    /// Starts a repeating timer.
    pub fn set_timer(&self, millis: u32) -> TimerId {
        self.core.backend().set_timer(self.core.window(), millis)
    }

    /// Stops a timer.
    pub fn kill_timer(&self, id: TimerId) {
        self.core.backend().kill_timer(self.core.window(), id);
    }

    /// Maps a timer tick to a message. Only one mapping can be installed;
    /// a widget that needs its own timer adds a listener instead.
    pub fn on_timer(&self, f: impl Fn(TimerId) -> Option<M> + 'static) {
        self.core.set_on_timer(f);
    }

    /// Adds a timer listener alongside the app's mapping, returning a token
    /// that [`Ui::remove_timer_listener`] uses to remove it. A widget that
    /// starts its own timer (a tooltip's show delay) observes the tick without
    /// taking over [`Ui::on_timer`].
    pub(crate) fn add_timer_listener(&self, f: impl Fn(TimerId) -> Option<M> + 'static) -> usize {
        self.core.add_timer_listener(f)
    }

    /// Removes the listener `token` returned by [`Ui::add_timer_listener`].
    pub(crate) fn remove_timer_listener(&self, token: usize) {
        self.core.remove_timer_listener(token);
    }

    /// Intercepts the close request: `Some(msg)` lets the app decide, `None`
    /// (the default) closes the window and quits.
    pub fn on_close(&self, f: impl Fn() -> Option<M> + 'static) {
        self.core.set_on_close(f);
    }

    /// Maps a shortcut key to the app's message, ahead of the focused widget.
    ///
    /// The mapper sees every `KeyDown` and returns `Some(msg)` to raise it (or
    /// `None` to ignore the key). The event is still delivered to the focused
    /// widget afterwards, so the mapper must only claim keys that widget leaves
    /// unhandled; a key it does handle (Ctrl+C in an editor) passes through
    /// because the mapper returns `None` for it.
    pub fn on_key(
        &self,
        f: impl Fn(crate::message::Key, crate::message::Modifiers) -> Option<M> + 'static,
    ) {
        self.core.set_on_key(f);
    }

    /// Maps a display-layout change to a message.
    pub fn on_display_change(&self, f: impl Fn() -> Option<M> + 'static) {
        self.core.set_on_display_change(f);
    }

    /// Maps a DPI change to a message. The mapper receives the new
    /// dots-per-inch (96 = 100%) and the backend's suggested window bounds in
    /// device pixels, or an empty [`Rect`] when the backend has no suggestion.
    ///
    /// The runtime does not move the window itself — a backend that can
    /// recommend bounds passes them through, but the application decides
    /// whether to adopt them. A window whose DPI changed must re-lay-out and
    /// repaint at the new value; [`Ui::dpi`] returns it.
    pub fn on_dpi_changed(&self, f: impl Fn(u32, Rect) -> Option<M> + 'static) {
        self.core.set_on_dpi_changed(f);
    }

    /// Closes the window.
    pub fn close(&self) {
        secondary::mark_closed(self.core.window());
        self.core.backend().close_window(self.core.window());
    }

    /// Closes the window with a value, which a modal opener receives from
    /// [`Ui::open_modal`]. On a non-modal window the value is discarded.
    pub fn close_with_result<R: 'static>(&self, result: R) {
        self.core.set_result(Box::new(result));
        self.close();
    }

    /// Ends the event loop.
    pub fn quit(&self) {
        self.core.backend().quit(0);
    }

    /// Ends the event loop with `code`.
    pub fn quit_with(&self, code: i32) {
        self.core.backend().quit(code);
    }
}

impl<M> Clone for Ui<M> {
    fn clone(&self) -> Ui<M> {
        Ui {
            core: Rc::clone(&self.core),
            parent: self.parent,
            design: Rc::clone(&self.design),
        }
    }
}
