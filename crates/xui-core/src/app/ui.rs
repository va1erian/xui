#![forbid(unsafe_code)]

//! The [`Ui`] handle: the widget layer's view of the window behind a [`Core`].

use std::rc::Rc;

use super::{Core, Proxy};
use crate::backend::{
    Cursor, Event, ImplKind, NodeKind, NodeSpec, Painter, ParentRef, Result, TextMetrics,
    TextStyle, TimerId, WidgetId,
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
    /// A container node this handle was scoped to, so widgets it creates become
    /// that container's children. `None` parents to the window.
    parent: Option<WidgetId>,
}

impl<M: 'static> Ui<M> {
    /// A handle over `core`.
    pub(crate) fn new(core: Rc<Core<M>>) -> Ui<M> {
        Ui { core, parent: None }
    }

    /// A handle scoped to the container `parent`: widgets created through it
    /// become children of that node, while messages and the window still belong
    /// to the top-level window.
    pub fn with_parent(&self, parent: WidgetId) -> Ui<M> {
        Ui {
            core: Rc::clone(&self.core),
            parent: Some(parent),
        }
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

    /// Creates a node from `spec`, parented to the container this handle is
    /// scoped to, or to the window.
    pub fn create_node(&self, spec: &NodeSpec) -> Result<WidgetId> {
        let parent = self
            .parent
            .map_or(ParentRef::Window(self.core.window()), ParentRef::Widget);
        self.core.backend().create(parent, spec)
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

    /// Raises a node above its siblings in the z-order.
    pub fn raise(&self, id: WidgetId) {
        self.core.backend().raise(id);
    }

    /// Requests the pointer shape shown over a node.
    pub fn set_cursor(&self, id: WidgetId, cursor: Cursor) {
        self.core.backend().set_cursor(id, cursor);
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

    /// A node's current bounds, in device pixels.
    pub fn bounds(&self, id: WidgetId) -> Rect {
        self.core.backend().bounds(id)
    }

    /// Whether the window is in design mode. In design mode a widget ignores its
    /// own input, so a form editor can select and move it.
    pub fn is_design_mode(&self) -> bool {
        self.core.design_mode()
    }

    /// Turns design mode on or off.
    pub fn set_design_mode(&self, on: bool) {
        self.core.set_design_mode(on);
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
            parent: self.parent,
        }
    }
}
