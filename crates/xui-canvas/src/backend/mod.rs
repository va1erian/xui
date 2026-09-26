#![forbid(unsafe_code)]

//! The cross-platform window backend: a `winit` window event loop that
//! composites the portable widgets with `tiny-skia` and presents them through
//! `softbuffer`.
//!
//! Nodes are kept in creation order (later created draws on top, and [`Backend::raise`] moves a
//! node to the back of that order). Bounds are parent-relative, as in the Win32
//! backend; the window composition walks the parent chain to place a node, and
//! input is translated into a node's own client coordinates before it is
//! delivered, matching the contract the portable widgets rely on.

mod app;
mod render;
#[cfg(test)]
mod tests;

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::time::Instant;

use winit::event_loop::{EventLoop, EventLoopProxy};
use winit::raw_window_handle::{
    DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, WindowHandle,
};
use winit::window::Window;

use xui_core::backend::{
    Backend, BackendError, Cursor, Decorations, ImplKind, NodeKind, NodeSpec, Painter, ParentRef,
    PlatformSpec, Result as BackendResult, TextMetrics, TextStyle, TimerId, Waker, WidgetId,
    WindowId,
};
use xui_core::router::WidgetHost;
use xui_core::{Dip, Rect, Theme};

/// A cloneable window handle for `softbuffer`. `winit::Window` is not `Clone`,
/// so the display and window handles share one `Rc`.
pub(crate) struct SharedWindow(pub(crate) Rc<Window>);

impl HasWindowHandle for SharedWindow {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        self.0.window_handle()
    }
}

impl HasDisplayHandle for SharedWindow {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        self.0.display_handle()
    }
}

/// The default dots-per-inch a window is rendered at before the platform
/// reports its scale factor.
const DEFAULT_DPI: u32 = 96;

/// A message posted to the event loop from another thread (a [`Waker`]) or the
/// backend itself.
pub(crate) enum UserEvent {
    /// Deliver [`xui_core::backend::Event::Wake`] to a window.
    Wake(WindowId),
}

/// One node the front layer created.
pub(crate) struct Node {
    pub(crate) window: WindowId,
    pub(crate) parent: ParentRef,
    pub(crate) bounds: Rect,
    pub(crate) visible: bool,
    pub(crate) enabled: bool,
    pub(crate) text: String,
    pub(crate) painter: Option<Painter>,
    /// Whether a press that starts here drags the whole window.
    pub(crate) drag_region: bool,
}

/// The per-window state.
pub(crate) struct WindowState {
    pub(crate) title: String,
    pub(crate) size: (u32, u32),
    pub(crate) dpi: u32,
    pub(crate) theme: Theme,
    pub(crate) sink: Option<Rc<dyn WidgetHost>>,
    pub(crate) window: Option<Rc<Window>>,
    /// Whether the window shows the system title bar.
    pub(crate) decorations: Decorations,
    /// The custom caption band height the app asked for.
    pub(crate) caption_inset: Dip,
    /// The node the pointer is over, so a `MouseLeave` can be sent on exit.
    pub(crate) hover: Option<WidgetId>,
    /// The node with the keyboard focus.
    pub(crate) focused: Option<WidgetId>,
}

impl WindowState {
    fn new(spec: &PlatformSpec) -> WindowState {
        WindowState {
            title: spec.title.clone(),
            size: (
                spec.width.to_px(DEFAULT_DPI).value().max(1) as u32,
                spec.height.to_px(DEFAULT_DPI).value().max(1) as u32,
            ),
            dpi: DEFAULT_DPI,
            theme: Theme::light(),
            sink: None,
            window: None,
            decorations: spec.decorations,
            caption_inset: spec.caption_inset,
            hover: None,
            focused: None,
        }
    }
}

/// State every backend method shares with the event-loop handler.
pub(crate) struct Shared {
    pub(crate) windows: RefCell<HashMap<u64, WindowState>>,
    pub(crate) nodes: RefCell<Vec<(WidgetId, Node)>>,
    pub(crate) cursors: RefCell<HashMap<u64, Cursor>>,
    /// Fires at the given instant, then is removed; carries its window.
    pub(crate) timers: RefCell<HashMap<usize, (WindowId, Instant)>>,
    pub(crate) next_window: Cell<u64>,
    pub(crate) next_widget: Cell<u64>,
    pub(crate) next_timer: Cell<usize>,
    pub(crate) proxy: EventLoopProxy<UserEvent>,
    pub(crate) exit_code: Cell<i32>,
    pub(crate) quit: Cell<bool>,
}

impl Shared {
    /// The top-level node under `(x, y)` in window coordinates, topmost first,
    /// with the point translated into that node's own client coordinates.
    pub(crate) fn hit_test(
        &self,
        window: WindowId,
        x: i32,
        y: i32,
    ) -> Option<(WidgetId, i32, i32)> {
        let nodes = self.nodes.borrow();
        for (id, node) in nodes.iter().rev() {
            if node.window != window || !node.visible || !node.enabled {
                continue;
            }
            let abs = render::absolute_bounds(&nodes, *id)?;
            if abs.contains(xui_core::Point::new(x, y)) {
                return Some((*id, x - abs.left, y - abs.top));
            }
        }
        None
    }

    /// Whether the node `id` is marked as a window-drag region.
    pub(crate) fn is_drag_region(&self, id: WidgetId) -> bool {
        self.nodes
            .borrow()
            .iter()
            .any(|(node_id, node)| *node_id == id && node.drag_region)
    }

    /// Sends `event` to the sink of `window`.
    pub(crate) fn deliver(
        &self,
        window: WindowId,
        target: WidgetId,
        event: &xui_core::backend::Event,
    ) -> bool {
        let sink = self
            .windows
            .borrow()
            .get(&window.raw())
            .and_then(|state| state.sink.clone());
        sink.is_some_and(|sink| sink.deliver(target, event))
    }

    /// Requests a repaint of `window`.
    pub(crate) fn request_redraw(&self, window: WindowId) {
        if let Some(state) = self.windows.borrow().get(&window.raw())
            && let Some(handle) = &state.window
        {
            handle.request_redraw();
        }
    }
}

/// The `winit` + `tiny-skia` backend.
pub struct WinitBackend {
    shared: Rc<Shared>,
    event_loop: RefCell<Option<EventLoop<UserEvent>>>,
}

impl Default for WinitBackend {
    fn default() -> WinitBackend {
        WinitBackend::new()
    }
}

impl WinitBackend {
    /// Creates the backend and its event loop. Must be called on the process's
    /// main thread; the event loop is consumed by [`Backend::run`].
    pub fn new() -> WinitBackend {
        let event_loop = EventLoop::<UserEvent>::with_user_event()
            .build()
            .expect("the winit event loop is created once on the main thread");
        let proxy = event_loop.create_proxy();
        WinitBackend {
            shared: Rc::new(Shared {
                windows: RefCell::new(HashMap::new()),
                nodes: RefCell::new(Vec::new()),
                cursors: RefCell::new(HashMap::new()),
                timers: RefCell::new(HashMap::new()),
                next_window: Cell::new(1),
                next_widget: Cell::new(1),
                next_timer: Cell::new(1),
                proxy,
                exit_code: Cell::new(0),
                quit: Cell::new(false),
            }),
            event_loop: RefCell::new(Some(event_loop)),
        }
    }

    fn allocate(cell: &Cell<u64>) -> u64 {
        let id = cell.get();
        cell.set(id + 1);
        id
    }
}

impl Backend for WinitBackend {
    fn run(&self) -> i32 {
        let Some(event_loop) = self.event_loop.take() else {
            return self.shared.exit_code.get();
        };
        app::run(event_loop, Rc::clone(&self.shared));
        self.shared.exit_code.get()
    }

    fn quit(&self, code: i32) {
        self.shared.exit_code.set(code);
        self.shared.quit.set(true);
        let _ = self
            .shared
            .proxy
            .send_event(UserEvent::Wake(WindowId::from_raw(0)));
    }

    fn wake(&self, window: WindowId) {
        let _ = self.shared.proxy.send_event(UserEvent::Wake(window));
    }

    fn waker(&self, window: WindowId) -> Waker {
        let proxy = self.shared.proxy.clone();
        Box::new(move || {
            let _ = proxy.send_event(UserEvent::Wake(window));
        })
    }

    fn set_event_sink(&self, window: WindowId, sink: Rc<dyn WidgetHost>) {
        if let Some(state) = self.shared.windows.borrow_mut().get_mut(&window.raw()) {
            state.sink = Some(sink);
        }
    }

    fn open_window(&self, spec: &PlatformSpec) -> BackendResult<WindowId> {
        let id = WindowId::from_raw(Self::allocate(&self.shared.next_window));
        self.shared
            .windows
            .borrow_mut()
            .insert(id.raw(), WindowState::new(spec));
        Ok(id)
    }

    fn close_window(&self, window: WindowId) {
        self.shared.windows.borrow_mut().remove(&window.raw());
        self.shared
            .nodes
            .borrow_mut()
            .retain(|(_, node)| node.window != window);
    }

    fn minimize(&self, window: WindowId) {
        self.window_handle(window, |handle| handle.set_minimized(true));
    }

    fn toggle_maximize(&self, window: WindowId) {
        self.window_handle(window, |handle| {
            handle.set_maximized(!handle.is_maximized());
        });
    }

    fn is_maximized(&self, window: WindowId) -> bool {
        self.window_handle(window, Window::is_maximized)
            .unwrap_or(false)
    }

    fn caption_inset(&self, window: WindowId) -> Dip {
        self.shared
            .windows
            .borrow()
            .get(&window.raw())
            .map_or(Dip(0.0), |state| state.caption_inset)
    }

    fn create(&self, parent: ParentRef, spec: &NodeSpec) -> BackendResult<WidgetId> {
        let window = match parent {
            ParentRef::Window(window) => window,
            ParentRef::Widget(widget) => self
                .shared
                .nodes
                .borrow()
                .iter()
                .find(|(id, _)| *id == widget)
                .map(|(_, node)| node.window)
                .ok_or(BackendError::CreateFailed("parent node"))?,
        };
        if !self.shared.windows.borrow().contains_key(&window.raw()) {
            return Err(BackendError::CreateFailed("window"));
        }
        let id = WidgetId::from_raw(Self::allocate(&self.shared.next_widget));
        self.shared.nodes.borrow_mut().push((
            id,
            Node {
                window,
                parent,
                bounds: spec.bounds,
                visible: spec.visible,
                enabled: spec.enabled,
                text: spec.text.clone(),
                painter: None,
                drag_region: false,
            },
        ));
        Ok(id)
    }

    fn destroy(&self, id: WidgetId) {
        let mut nodes = self.shared.nodes.borrow_mut();
        nodes.retain(|(node_id, _)| *node_id != id);
        loop {
            let gone: Vec<WidgetId> = nodes
                .iter()
                .filter(|(_, node)| match node.parent {
                    ParentRef::Window(window) => {
                        !self.shared.windows.borrow().contains_key(&window.raw())
                    }
                    ParentRef::Widget(parent) => !nodes.iter().any(|(id, _)| *id == parent),
                })
                .map(|(id, _)| *id)
                .collect();
            if gone.is_empty() {
                break;
            }
            nodes.retain(|(id, _)| !gone.contains(id));
        }
    }

    fn apply_moves(&self, _window: WindowId, moves: &[(WidgetId, Rect)]) {
        let mut nodes = self.shared.nodes.borrow_mut();
        for (id, rect) in moves {
            if let Some((_, node)) = nodes.iter_mut().find(|(node_id, _)| node_id == id) {
                node.bounds = *rect;
            }
        }
    }

    fn set_visible(&self, id: WidgetId, visible: bool) {
        self.with_node(id, |node| node.visible = visible);
    }

    fn set_enabled(&self, id: WidgetId, enabled: bool) {
        self.with_node(id, |node| node.enabled = enabled);
    }

    fn raise(&self, id: WidgetId) {
        let mut nodes = self.shared.nodes.borrow_mut();
        if let Some(at) = nodes.iter().position(|(node_id, _)| *node_id == id) {
            let entry = nodes.remove(at);
            nodes.push(entry);
        }
    }

    fn set_drag_region(&self, id: WidgetId, drag: bool) {
        self.with_node(id, |node| node.drag_region = drag);
    }

    fn set_cursor(&self, id: WidgetId, cursor: Cursor) {
        self.shared.cursors.borrow_mut().insert(id.raw(), cursor);
    }

    fn focus(&self, id: WidgetId) {
        let window = self
            .shared
            .nodes
            .borrow()
            .iter()
            .find(|(node_id, _)| *node_id == id)
            .map(|(_, node)| node.window);
        let Some(window) = window else {
            return;
        };
        let previous = {
            let mut windows = self.shared.windows.borrow_mut();
            let Some(state) = windows.get_mut(&window.raw()) else {
                return;
            };
            state.focused.replace(id)
        };
        if let Some(previous) = previous
            && previous != id
        {
            self.shared
                .deliver(window, previous, &xui_core::backend::Event::KillFocus);
        }
        self.shared
            .deliver(window, id, &xui_core::backend::Event::SetFocus);
    }

    fn set_text(&self, id: WidgetId, text: &str) {
        self.with_node(id, |node| node.text = text.to_string());
    }

    fn text(&self, id: WidgetId) -> String {
        self.shared
            .nodes
            .borrow()
            .iter()
            .find(|(node_id, _)| *node_id == id)
            .map(|(_, node)| node.text.clone())
            .unwrap_or_default()
    }

    fn bounds(&self, id: WidgetId) -> Rect {
        self.shared
            .nodes
            .borrow()
            .iter()
            .find(|(node_id, _)| *node_id == id)
            .map_or(Rect::default(), |(_, node)| node.bounds)
    }

    fn invalidate(&self, id: WidgetId) {
        if let Some(window) = self.window_of(id) {
            self.shared.request_redraw(window);
        }
    }

    fn invalidate_rect(&self, id: WidgetId, _rect: Rect) {
        self.invalidate(id);
    }

    fn set_painter(&self, id: WidgetId, painter: Painter) {
        self.with_node(id, |node| node.painter = Some(painter));
    }

    fn measure_text(&self, text: &str, style: &TextStyle, dpi: u32) -> TextMetrics {
        crate::text::measure(text, style, dpi, i32::MAX)
    }

    fn dpi(&self, window: WindowId) -> u32 {
        self.shared
            .windows
            .borrow()
            .get(&window.raw())
            .map_or(DEFAULT_DPI, |state| state.dpi)
    }

    fn client_rect(&self, window: WindowId) -> Rect {
        self.shared
            .windows
            .borrow()
            .get(&window.raw())
            .map_or(Rect::default(), |state| {
                Rect::new(0, 0, state.size.0 as i32, state.size.1 as i32)
            })
    }

    fn set_theme(&self, window: WindowId, theme: &Theme) {
        if let Some(state) = self.shared.windows.borrow_mut().get_mut(&window.raw()) {
            state.theme = *theme;
        }
    }

    fn set_timer(&self, window: WindowId, millis: u32) -> TimerId {
        let id = self.shared.next_timer.get();
        self.shared.next_timer.set(id + 1);
        self.shared.timers.borrow_mut().insert(
            id,
            (
                window,
                Instant::now() + std::time::Duration::from_millis(u64::from(millis.max(1))),
            ),
        );
        TimerId(id)
    }

    fn kill_timer(&self, _window: WindowId, id: TimerId) {
        self.shared.timers.borrow_mut().remove(&id.0);
    }

    fn supports(&self, _kind: NodeKind) -> ImplKind {
        ImplKind::Painted
    }
}

impl WinitBackend {
    /// Runs `f` with `window`'s live `winit::Window`, if it has one yet.
    fn window_handle<R>(&self, window: WindowId, f: impl FnOnce(&Window) -> R) -> Option<R> {
        let handle = self
            .shared
            .windows
            .borrow()
            .get(&window.raw())
            .and_then(|state| state.window.clone())?;
        Some(f(&handle))
    }

    fn with_node(&self, id: WidgetId, f: impl FnOnce(&mut Node)) {
        if let Some((_, node)) = self
            .shared
            .nodes
            .borrow_mut()
            .iter_mut()
            .find(|(node_id, _)| *node_id == id)
        {
            f(node);
        }
    }

    fn window_of(&self, id: WidgetId) -> Option<WindowId> {
        self.shared
            .nodes
            .borrow()
            .iter()
            .find(|(node_id, _)| *node_id == id)
            .map(|(_, node)| node.window)
    }
}
