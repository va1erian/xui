#![forbid(unsafe_code)]

//! The contract over painted child windows.

use std::cell::Cell;
use std::rc::Rc;

use xui_core::backend::{
    Backend, BackendError, Cursor, FontSpec, ImplKind, NodeKind, NodeSpec, Painter, ParentRef,
    PlatformSpec, Result as BackendResult, TextLayout, TextMetrics, TextShaper, TextStyle, TimerId,
    Waker, WidgetId, WindowId,
};
use xui_core::router::WidgetHost;
use xui_core::{Dip, Px, Rect, Theme};

use super::handler::{TopHandler, WindowShared};
use super::node::BackendNode;
use super::{BackendWindow, Win32Backend, chrome, cursor::cursor_shape, text};
use crate::sys;
use crate::window::{Window, WindowClass, WindowExStyle, WindowStyle};

/// The contract over painted child windows.
///
/// A node's text is measured and drawn with the same styled-text path, so
/// [`TextStyle`]'s family, size, weight, slant and colour are honoured and
/// measurement agrees with painting.
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
        // The spec is in Dip and the window does not exist yet, so convert at
        // the process-wide system DPI; once it exists `dpi()` reads the
        // monitor it landed on.
        let dpi = sys::dpi::system_dpi();
        let bounds = Rect::new(
            0,
            0,
            spec.width.to_px(dpi).value(),
            spec.height.to_px(dpi).value(),
        );
        let class = WindowClass::register("xui.backend", background)
            .map_err(|_| BackendError::CreateFailed("window class"))?;
        let window = Window::create(
            class,
            None,
            WindowStyle::overlapped().clip_children(),
            WindowExStyle::new(),
            bounds,
            &spec.title,
            TopHandler::new(id, Rc::clone(&shared)),
        )
        .map_err(|_| BackendError::CreateFailed("window"))?;
        chrome::apply(spec, &window);
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

    fn minimize(&self, window: WindowId) {
        let hwnd = self.window_hwnd(window);
        if let Some(hwnd) = hwnd {
            sys::window::show(hwnd, sys::window::ShowKind::Minimized);
        }
    }

    fn toggle_maximize(&self, window: WindowId) {
        let Some(hwnd) = self.window_hwnd(window) else {
            return;
        };
        let kind = if sys::window::is_maximized(hwnd) {
            sys::window::ShowKind::Normal
        } else {
            sys::window::ShowKind::Maximized
        };
        sys::window::show(hwnd, kind);
    }

    fn is_maximized(&self, window: WindowId) -> bool {
        self.windows
            .borrow()
            .get(&window.raw())
            .is_some_and(|entry| sys::window::is_maximized(entry.window.hwnd()))
    }

    fn caption_inset(&self, window: WindowId) -> Dip {
        let Some(hwnd) = self.window_hwnd(window) else {
            return Dip(0.0);
        };
        if !crate::window::nc::is_extended(hwnd) {
            return Dip(0.0);
        }
        Px(sys::nc::title_bar_height(hwnd)).to_dip(self.dpi(window))
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

    fn set_drag_region(&self, id: WidgetId, drag: bool) {
        if let Some(node) = self.nodes.borrow().get(&id.raw()) {
            node.drag_region.set(drag);
        }
    }

    fn set_cursor(&self, id: WidgetId, cursor: Cursor) {
        if let Some((hwnd, _)) = self.node(id) {
            sys::window_input::set_cursor(hwnd, cursor_shape(cursor));
        }
    }

    /// Per-node clipping is a no-op on Win32: a painted node is a real child
    /// window, so Windows already clips it to its parent's client rectangle
    /// (the same result the portable clip expresses). A clip *smaller* than a
    /// node would need `SetWindowRgn`, which would also clip the node's own
    /// scrollbar sibling; no portable widget asks for one.
    fn set_clip(&self, id: WidgetId, rect: Option<Rect>) {
        let _ = (id, rect);
    }

    fn set_capture(&self, id: WidgetId) {
        if let Some((hwnd, _)) = self.node(id) {
            sys::window_input::set_capture(hwnd);
        }
    }

    fn release_capture(&self) {
        sys::window_input::release_capture();
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

    fn measure_text(&self, text: &str, style: &TextStyle, dpi: u32) -> TextMetrics {
        text::measure_d2d(text, style, dpi).unwrap_or_else(|| text::measure_gdi(text, style, dpi))
    }

    fn text_shaper(&self) -> Box<dyn TextShaper> {
        Box::new(text::Win32TextShaper::new())
    }

    fn layout_text(
        &self,
        text: &str,
        spec: &FontSpec,
        max_width: f32,
        dpi: u32,
    ) -> Box<dyn TextLayout> {
        text::layout_text(text, spec, max_width, dpi)
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
            let hwnd = entry.window.hwnd();
            sys::set_class_background(hwnd, theme.background);
            sys::set_titlebar_dark(hwnd, theme.is_dark);
            if crate::window::nc::is_extended(hwnd) {
                sys::apply_extended_colors(hwnd, theme, crate::theme::backdrop_active(hwnd));
            }
            // Painters read the shared theme live, but they only repaint when
            // asked, so invalidate the whole tree (children included).
            sys::window::redraw_children(hwnd);
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
