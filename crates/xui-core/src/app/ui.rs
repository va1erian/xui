#![forbid(unsafe_code)]

//! The [`Ui`] handle: the widget layer's view of the window behind a [`Core`].

use std::rc::Rc;

use super::{Core, Proxy};
use crate::backend::{
    Event, ImplKind, NodeKind, NodeSpec, Painter, ParentRef, Result, TextMetrics, TextStyle,
    TimerId, WidgetId,
};
use crate::geometry::Rect;
use crate::theme::Theme;

/// The widget layer's handle to a top-level window.
///
/// Cheap to clone (an [`Rc`]). Widget constructors take it, create their node
/// through it, and map their node's events to the app's `Msg`. It deliberately
/// exposes only the portable backend operations, so a widget never names a
/// platform type; [`Ui::backend`] is the crate-internal seam for a backend to
/// add its own widgets.
pub struct Ui<M> {
    core: Rc<Core<M>>,
}

impl<M: 'static> Ui<M> {
    /// A handle over `core`.
    pub(crate) fn new(core: Rc<Core<M>>) -> Ui<M> {
        Ui { core }
    }

    /// The window this handle drives.
    pub fn window(&self) -> crate::backend::WindowId {
        self.core.window()
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

    /// Creates a node parented to the window from `spec`.
    pub fn create_node(&self, spec: &NodeSpec) -> Result<WidgetId> {
        self.core
            .backend()
            .create(ParentRef::Window(self.core.window()), spec)
    }

    /// Creates a node inside the container `parent`.
    pub fn create_child(&self, parent: WidgetId, spec: &NodeSpec) -> Result<WidgetId> {
        self.core.backend().create(ParentRef::Widget(parent), spec)
    }

    /// Destroys a node and every node inside it, and forgets its event mapper.
    ///
    /// Unregistering first matters: a mapper captures the `Ui` (and so the
    /// `Core`), and leaving it in the router would keep the whole window alive.
    pub fn destroy(&self, id: WidgetId) {
        self.core.router().unregister(id);
        self.core.backend().destroy(id);
    }

    /// Moves several nodes as one batch.
    pub fn apply_moves(&self, moves: &[(WidgetId, Rect)]) {
        self.core.backend().apply_moves(self.core.window(), moves);
    }

    /// Shows or hides a node.
    pub fn set_visible(&self, id: WidgetId, visible: bool) {
        self.core.backend().set_visible(id, visible);
    }

    /// Enables or disables a node.
    pub fn set_enabled(&self, id: WidgetId, enabled: bool) {
        self.core.backend().set_enabled(id, enabled);
    }

    /// Gives a node the keyboard focus.
    pub fn focus(&self, id: WidgetId) {
        self.core.backend().focus(id);
    }

    /// Replaces a node's text.
    pub fn set_text(&self, id: WidgetId, text: &str) {
        self.core.backend().set_text(id, text);
    }

    /// A node's current text (a native control answers from its own state).
    pub fn text(&self, id: WidgetId) -> String {
        self.core.backend().text(id)
    }

    /// Schedules a repaint of a node.
    pub fn invalidate(&self, id: WidgetId) {
        self.core.backend().invalidate(id);
    }

    /// Schedules a repaint of `rect` within a node.
    pub fn invalidate_rect(&self, id: WidgetId, rect: Rect) {
        self.core.backend().invalidate_rect(id, rect);
    }

    /// Registers the draw routine for a painted node.
    pub fn set_painter(&self, id: WidgetId, painter: Painter) {
        self.core.backend().set_painter(id, painter);
    }

    /// Measures a run of text in device pixels.
    pub fn measure_text(&self, text: &str, style: &TextStyle, dpi: u32) -> TextMetrics {
        self.core.backend().measure_text(text, style, dpi)
    }

    /// Whether the backend provides a native widget for `kind`.
    pub fn supports(&self, kind: NodeKind) -> ImplKind {
        self.core.backend().supports(kind)
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

    /// Maps a timer tick to a message. Only one mapping can be installed.
    pub fn on_timer(&self, f: impl Fn(TimerId) -> Option<M> + 'static) {
        self.core.set_on_timer(f);
    }

    /// Intercepts the close request: `Some(msg)` lets the app decide, `None`
    /// (the default) closes the window and quits.
    pub fn on_close(&self, f: impl Fn() -> Option<M> + 'static) {
        self.core.set_on_close(f);
    }

    /// Maps a display-layout change to a message.
    pub fn on_display_change(&self, f: impl Fn() -> Option<M> + 'static) {
        self.core.set_on_display_change(f);
    }

    /// Closes the window.
    pub fn close(&self) {
        self.core.backend().close_window(self.core.window());
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
        }
    }
}
