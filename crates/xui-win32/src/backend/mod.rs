#![forbid(unsafe_code)]

//! The Win32 implementation of the [`Backend`] contract.
//!
//! Most nodes are painted child windows: the front layer registers a painter
//! and the node's handler runs it on `WM_PAINT`. Kinds with a good native
//! control are hosted natively instead — today [`NodeKind::Edit`] is a real
//! `EDIT`, whose changes reach the widget as [`Event::TextChanged`] — and
//! [`Win32Backend::supports`] reports [`ImplKind::Native`] for them.

mod canvas;
mod handler;
mod node;

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use xui_core::backend::{
    Backend, BackendError, Cursor, ImplKind, NodeKind, NodeSpec, Painter, ParentRef, PlatformSpec,
    Result as BackendResult, TextMetrics, TextStyle, TimerId, Waker, WidgetId, WindowId,
};
use xui_core::router::WidgetHost;
use xui_core::{Rect, Theme};

use crate::gdi::Font;
use crate::sys;
use crate::window::{Window, WindowClass, WindowExStyle, WindowStyle};

use handler::{TopHandler, WindowShared};
use node::BackendNode;

/// A top-level window the backend created.
struct BackendWindow {
    window: Window,
    shared: Rc<WindowShared>,
    theme: Cell<Theme>,
}

/// The Win32 backend.
pub struct Win32Backend {
    windows: RefCell<HashMap<u64, BackendWindow>>,
    nodes: RefCell<HashMap<u64, BackendNode>>,
    next_window: Cell<u64>,
    next_widget: Cell<u64>,
}

impl Default for Win32Backend {
    fn default() -> Win32Backend {
        Win32Backend::new()
    }
}

impl Win32Backend {
    /// A backend with no windows yet. Ids start at one so the first handle is
    /// never the null id.
    pub fn new() -> Win32Backend {
        Win32Backend {
            windows: RefCell::new(HashMap::new()),
            nodes: RefCell::new(HashMap::new()),
            next_window: Cell::new(1),
            next_widget: Cell::new(1),
        }
    }

    fn allocate(cell: &Cell<u64>) -> u64 {
        let id = cell.get();
        cell.set(id + 1);
        id
    }

    /// Resolves a parent to the handle to parent under, its owning window, and
    /// that window's shared state.
    fn resolve_parent(
        &self,
        parent: ParentRef,
    ) -> BackendResult<(crate::hwnd::Hwnd, WindowId, Rc<WindowShared>)> {
        match parent {
            ParentRef::Window(window) => {
                let windows = self.windows.borrow();
                let entry = windows
                    .get(&window.raw())
                    .ok_or(BackendError::CreateFailed("window"))?;
                Ok((entry.window.hwnd(), window, Rc::clone(&entry.shared)))
            }
            ParentRef::Widget(widget) => {
                let (hwnd, window) = {
                    let nodes = self.nodes.borrow();
                    let node = nodes
                        .get(&widget.raw())
                        .ok_or(BackendError::CreateFailed("parent node"))?;
                    (node.hwnd, node.window_id)
                };
                let shared = {
                    let windows = self.windows.borrow();
                    let entry = windows
                        .get(&window.raw())
                        .ok_or(BackendError::CreateFailed("window"))?;
                    Rc::clone(&entry.shared)
                };
                Ok((hwnd, window, shared))
            }
        }
    }

    fn node(&self, id: WidgetId) -> Option<(crate::hwnd::Hwnd, WindowId)> {
        self.nodes
            .borrow()
            .get(&id.raw())
            .map(|node| (node.hwnd, node.window_id))
    }

    /// Forgets a node's child handle in its window's index, so a later
    /// `WM_COMMAND` cannot reach a destroyed widget.
    fn unregister(&self, id: &WidgetId) {
        let entry = {
            let nodes = self.nodes.borrow();
            nodes.get(&id.raw()).map(|node| (node.hwnd, node.window_id))
        };
        if let Some((hwnd, window_id)) = entry
            && let Some(window) = self.windows.borrow().get(&window_id.raw())
        {
            window.shared.unregister_node(hwnd);
        }
    }

    /// The handle behind a top-level window, for interop with the platform
    /// layer (capture, message hooks). `None` once it is closed.
    pub fn window_hwnd(&self, window: WindowId) -> Option<crate::hwnd::Hwnd> {
        self.windows
            .borrow()
            .get(&window.raw())
            .map(|entry| entry.window.hwnd())
    }

    /// The handle behind a node, for interop with the platform layer.
    pub fn node_hwnd(&self, id: WidgetId) -> Option<crate::hwnd::Hwnd> {
        self.node(id).map(|(hwnd, _)| hwnd)
    }
}

/// The contract over painted child windows.
///
/// Text is measured and drawn with the shared UI font for now;
/// [`TextStyle`]'s size and weight are not honoured yet, so measurement and
/// drawing stay consistent until native font selection lands with the
/// controls.
impl Backend for Win32Backend {
    fn init(&self) {
        crate::init();
    }

    fn run(&self) -> i32 {
        crate::looper::run()
    }

    fn quit(&self, code: i32) {
        crate::looper::quit(code);
    }

    fn wake(&self, window: WindowId) {
        if let Some(entry) = self.windows.borrow().get(&window.raw()) {
            let _ = entry.window.post_wake();
        }
    }

    fn waker(&self, window: WindowId) -> Waker {
        let target = self
            .windows
            .borrow()
            .get(&window.raw())
            .map(|entry| entry.window.hwnd());
        match target {
            Some(hwnd) => {
                let wake = sys::message::wake_message();
                // Posting to a window handle is thread-safe, so a worker can
                // call this.
                Box::new(move || {
                    let _ = sys::window::post_message(hwnd, wake, 0, 0);
                })
            }
            None => Box::new(|| {}),
        }
    }

    fn set_event_sink(&self, window: WindowId, sink: Rc<dyn WidgetHost>) {
        if let Some(entry) = self.windows.borrow().get(&window.raw()) {
            entry.shared.set_sink(sink);
        }
    }

    fn open_window(&self, spec: &PlatformSpec) -> BackendResult<WindowId> {
        let id = WindowId::from_raw(Self::allocate(&self.next_window));
        let shared = WindowShared::new();
        let background = Theme::light().background;
        let bounds = Rect::new(
            0,
            0,
            spec.width.to_px(96).value(),
            spec.height.to_px(96).value(),
        );
        let class = WindowClass::register("xui.backend", background)
            .map_err(|_| BackendError::CreateFailed("window class"))?;
        let window = Window::create(
            class,
            None,
            WindowStyle::overlapped(),
            WindowExStyle::new(),
            bounds,
            &spec.title,
            TopHandler::new(id, Rc::clone(&shared)),
        )
        .map_err(|_| BackendError::CreateFailed("window"))?;
        window.show();
        self.windows.borrow_mut().insert(
            id.raw(),
            BackendWindow {
                window,
                shared,
                theme: Cell::new(Theme::light()),
            },
        );
        Ok(id)
    }

    fn close_window(&self, window: WindowId) {
        self.windows.borrow_mut().remove(&window.raw());
        self.nodes
            .borrow_mut()
            .retain(|_, node| node.window_id != window);
    }

    fn create(&self, parent: ParentRef, spec: &NodeSpec) -> BackendResult<WidgetId> {
        let (parent_hwnd, window_id, shared) = self.resolve_parent(parent)?;
        let widget = WidgetId::from_raw(Self::allocate(&self.next_widget));
        let node = BackendNode::create(window_id, parent_hwnd, parent, &shared, widget, spec)?;
        self.nodes.borrow_mut().insert(widget.raw(), node);
        Ok(widget)
    }

    fn destroy(&self, id: WidgetId) {
        self.unregister(&id);
        self.nodes.borrow_mut().remove(&id.raw());
        // Cascade: a container's children may be destroyed with it, so drop
        // every node whose parent chain no longer exists.
        loop {
            let doomed: Vec<u64> = {
                let nodes = self.nodes.borrow();
                nodes
                    .iter()
                    .filter(|(_, node)| match node.parent {
                        ParentRef::Window(window) => {
                            !self.windows.borrow().contains_key(&window.raw())
                        }
                        ParentRef::Widget(parent) => !nodes.contains_key(&parent.raw()),
                    })
                    .map(|(id, _)| *id)
                    .collect()
            };
            if doomed.is_empty() {
                break;
            }
            for id in &doomed {
                self.unregister(&WidgetId::from_raw(*id));
            }
            let mut nodes = self.nodes.borrow_mut();
            for id in doomed {
                nodes.remove(&id);
            }
        }
    }

    fn apply_moves(&self, _window: WindowId, moves: &[(WidgetId, Rect)]) {
        let os_moves: Vec<(crate::hwnd::Hwnd, Rect)> = {
            let nodes = self.nodes.borrow();
            for (id, rect) in moves {
                if let Some(node) = nodes.get(&id.raw()) {
                    node.set_bounds(*rect);
                }
            }
            moves
                .iter()
                .filter_map(|(id, rect)| nodes.get(&id.raw()).map(|node| (node.hwnd, *rect)))
                .collect()
        };
        sys::layout::apply(&os_moves);
    }

    fn set_visible(&self, id: WidgetId, visible: bool) {
        if let Some((hwnd, _)) = self.node(id) {
            let kind = if visible {
                sys::window::ShowKind::Normal
            } else {
                sys::window::ShowKind::Hidden
            };
            sys::window::show(hwnd, kind);
        }
    }

    fn set_enabled(&self, id: WidgetId, enabled: bool) {
        if let Some((hwnd, _)) = self.node(id) {
            sys::window::enable_window(hwnd, enabled);
        }
    }

    fn raise(&self, id: WidgetId) {
        if let Some((hwnd, _)) = self.node(id) {
            sys::window::bring_to_top(hwnd);
        }
    }

    fn set_cursor(&self, id: WidgetId, cursor: Cursor) {
        if let Some(node) = self.nodes.borrow().get(&id.raw()) {
            node.cursor.set(cursor);
        }
    }

    fn focus(&self, id: WidgetId) {
        if let Some((hwnd, _)) = self.node(id) {
            sys::window::set_focus(hwnd);
        }
    }

    fn set_text(&self, id: WidgetId, text: &str) {
        if let Some(node) = self.nodes.borrow().get(&id.raw()) {
            node.set_text(text);
        }
    }

    fn text(&self, id: WidgetId) -> String {
        self.nodes
            .borrow()
            .get(&id.raw())
            .map_or_else(String::new, BackendNode::text)
    }

    fn bounds(&self, id: WidgetId) -> Rect {
        self.nodes
            .borrow()
            .get(&id.raw())
            .map_or_else(Rect::default, BackendNode::bounds)
    }

    fn invalidate(&self, id: WidgetId) {
        if let Some((hwnd, _)) = self.node(id) {
            sys::window::invalidate(hwnd);
        }
    }

    fn invalidate_rect(&self, id: WidgetId, rect: Rect) {
        if let Some((hwnd, _)) = self.node(id) {
            sys::window::invalidate_rect(hwnd, rect);
        }
    }

    fn set_painter(&self, id: WidgetId, painter: Painter) {
        if let Some(node) = self.nodes.borrow().get(&id.raw()) {
            node.painter.replace(Some(painter));
        }
    }

    fn measure_text(&self, text: &str, _style: &TextStyle, dpi: u32) -> TextMetrics {
        let size = Font::shared_ui(dpi)
            .map(|font| sys::gdi::measure_text(font.raw(), text))
            .unwrap_or_default();
        TextMetrics {
            width: size.width,
            height: size.height,
            ascent: size.height * 3 / 4,
            descent: size.height / 4,
        }
    }

    fn dpi(&self, window: WindowId) -> u32 {
        self.windows
            .borrow()
            .get(&window.raw())
            .map_or(96, |entry| entry.window.dpi())
    }

    fn client_rect(&self, window: WindowId) -> Rect {
        self.windows
            .borrow()
            .get(&window.raw())
            .map_or(Rect::default(), |entry| entry.window.client_rect())
    }

    fn set_theme(&self, window: WindowId, theme: &Theme) {
        if let Some(entry) = self.windows.borrow().get(&window.raw()) {
            entry.theme.set(*theme);
            entry.shared.set_theme(*theme);
            sys::set_class_background(entry.window.hwnd(), theme.background);
            // Painters read the shared theme live, but they only repaint when
            // asked, so invalidate the whole tree (children included).
            sys::window::redraw_children(entry.window.hwnd());
        }
    }

    fn set_timer(&self, window: WindowId, millis: u32) -> TimerId {
        self.windows
            .borrow()
            .get(&window.raw())
            .and_then(|entry| entry.window.set_timer(millis).ok())
            .unwrap_or(TimerId(0))
    }

    fn kill_timer(&self, window: WindowId, id: TimerId) {
        if let Some(entry) = self.windows.borrow().get(&window.raw()) {
            entry.window.kill_timer(id);
        }
    }

    fn supports(&self, kind: NodeKind) -> ImplKind {
        BackendNode::impl_kind(kind)
    }
}
