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
use xui_core::{Dip, Px, Rect, Theme};

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

    fn set_window_icon(&self, window: WindowId, icon: &xui_core::image::Image) {
        let (Ok(width), Ok(height)) = (i32::try_from(icon.width()), i32::try_from(icon.height()))
        else {
            return;
        };
        let Ok(icon) = crate::Icon::from_rgba(width, height, icon.pixels()) else {
            return;
        };
        if let Some(entry) = self.windows.borrow().get(&window.raw()) {
            entry.window.set_icon(&icon);
            *entry.icon.borrow_mut() = Some(icon);
        }
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
        self.create_node(parent, spec)
    }

    fn destroy(&self, id: WidgetId) {
        self.destroy_node(id);
    }

    fn apply_moves(&self, window: WindowId, moves: &[(WidgetId, Rect)]) {
        self.apply_node_moves(window, moves);
    }

    fn set_visible(&self, id: WidgetId, visible: bool) {
        self.set_node_visible(id, visible);
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
        self.focus_node(id);
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

    fn clipboard_text(&self) -> Option<String> {
        // `OpenClipboard` needs a live owner window; any open one will do.
        let hwnd = self
            .windows
            .borrow()
            .values()
            .next()
            .map(|entry| entry.window.hwnd())?;
        crate::clipboard::text(hwnd).ok().flatten()
    }

    fn set_clipboard_text(&self, text: &str) {
        let hwnd = self
            .windows
            .borrow()
            .values()
            .next()
            .map(|entry| entry.window.hwnd());
        if let Some(hwnd) = hwnd {
            let _ = crate::clipboard::set_text(hwnd, text);
        }
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
