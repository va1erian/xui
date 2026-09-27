#![forbid(unsafe_code)]

//! The contract over painted child windows.

use std::rc::Rc;

use xui_core::backend::{
    Backend, Cursor, FontSpec, ImplKind, NativeWindowHandle, NodeKind, NodeSpec, Painter,
    ParentRef, PlatformSpec, Result as BackendResult, TextLayout, TextMetrics, TextShaper,
    TextStyle, TimerId, Waker, WidgetId, WindowId,
};
use xui_core::image::Image;
use xui_core::router::WidgetHost;
use xui_core::{Dip, Point, Px, Rect, Theme};

use super::handler::WindowShared;
use super::node::BackendNode;
use super::{Win32Backend, cursor::cursor_shape, text};
use crate::sys;

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
        self.open(spec)
    }

    fn close_window(&self, window: WindowId) {
        self.windows.borrow_mut().remove(&window.raw());
        self.nodes
            .borrow_mut()
            .retain(|_, node| node.window_id != window);
    }

    fn set_window_title(&self, window: WindowId, title: &str) {
        self.set_title(window, title);
    }

    fn set_window_enabled(&self, window: WindowId, enabled: bool) {
        self.set_enabled(window, enabled);
    }

    fn native_window(&self, window: WindowId) -> Option<NativeWindowHandle> {
        self.native_handle(window)
    }

    fn capture(&self, window: WindowId) -> BackendResult<Image> {
        self.capture_image(window)
    }

    fn run_modal(&self, window: WindowId) -> BackendResult<()> {
        self.run_modal_loop(window)
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
            let windows = self.windows.borrow();
            for (id, rect) in moves {
                if let Some(node) = nodes.get(&id.raw()) {
                    node.set_bounds(*rect);
                }
            }
            moves
                .iter()
                .filter_map(|(id, rect)| {
                    let node = nodes.get(&id.raw())?;
                    // A popup is a top-level window: the core works in host
                    // client coordinates, so convert its rect to the screen.
                    let rect = if node.is_popup {
                        let entry = windows.get(&node.window_id.raw())?;
                        // Track the host-client rect so the popup follows the
                        // host window on a move.
                        entry.shared.track_popup(node.hwnd, *rect);
                        let at = sys::window::client_to_screen(
                            entry.window.hwnd(),
                            Point::new(rect.left, rect.top),
                        );
                        Rect::new(at.x, at.y, at.x + rect.width(), at.y + rect.height())
                    } else {
                        *rect
                    };
                    Some((node.hwnd, rect))
                })
                .collect()
        };
        sys::layout::apply(&os_moves);
    }

    fn set_visible(&self, id: WidgetId, visible: bool) {
        let entry = self
            .nodes
            .borrow()
            .get(&id.raw())
            .map(|node| (node.hwnd, node.is_popup));
        if let Some((hwnd, is_popup)) = entry {
            if visible && is_popup {
                // Paint the finished face before the popup is composed, so its
                // first frame is never the class background. Show it without
                // activating: a plain `SW_SHOW` would steal the host's
                // activation and flicker its chrome on every open/switch.
                sys::first_show::show_painted(hwnd, || {
                    sys::window::show(hwnd, sys::window::ShowKind::NoActivate);
                });
            } else {
                let kind = if visible {
                    sys::window::ShowKind::Normal
                } else {
                    sys::window::ShowKind::Hidden
                };
                sys::window::show(hwnd, kind);
            }
        }
        if !visible {
            self.clear_key_focus(id);
            if let Some((hwnd, true)) = entry
                && let Some(shared) = self.node_shared(id)
            {
                shared.forget_popup(hwnd);
            }
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
        let Some((hwnd, is_popup, window)) = self
            .nodes
            .borrow()
            .get(&id.raw())
            .map(|node| (node.hwnd, node.is_popup, node.window_id))
        else {
            return;
        };
        let shared = self
            .windows
            .borrow()
            .get(&window.raw())
            .map(|entry| Rc::clone(&entry.shared));
        let Some(shared) = shared else {
            return;
        };
        if is_popup {
            // A top-level popup must not take activation, or the host window
            // would lose focus. Keep the OS focus on the host and route its
            // keyboard input to the popup instead, as a native menu does.
            shared.set_key_focus(Some(id));
            if let Some(host) = self.window_hwnd(window) {
                sys::window::set_focus(host);
            }
        } else {
            shared.set_key_focus(None);
            sys::window::set_focus(hwnd);
        }
    }

    fn set_text(&self, id: WidgetId, text: &str) {
        if let Some(node) = self.nodes.borrow().get(&id.raw()) {
            node.set_text(text);
        }
    }

    fn set_cue(&self, id: WidgetId, cue: &str) {
        if let Some(node) = self.nodes.borrow().get(&id.raw()) {
            node.set_cue(cue);
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
        self.apply_theme(window, theme);
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

impl Win32Backend {
    /// The shared state of the window that owns `id`, if it still exists.
    fn node_shared(&self, id: WidgetId) -> Option<Rc<WindowShared>> {
        let window = self
            .nodes
            .borrow()
            .get(&id.raw())
            .map(|node| node.window_id)?;
        self.windows
            .borrow()
            .get(&window.raw())
            .map(|entry| Rc::clone(&entry.shared))
    }

    /// Drops the host window's logical keyboard focus when it is `id`, so a
    /// hidden or destroyed popup stops receiving the host's key input.
    fn clear_key_focus(&self, id: WidgetId) {
        if let Some(shared) = self.node_shared(id) {
            shared.clear_key_focus(id);
        }
    }
}
