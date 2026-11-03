#![forbid(unsafe_code)]

//! The [`Backend`] contract over the window procedure and the scene.

use std::rc::Rc;

use xui_core::backend::{
    Backend, BackendError, Cursor, Event, FontSpec, ImplKind, NodeKind, NodeSpec, Painter,
    ParentRef, PlatformSpec, Result, TextLayout, TextMetrics, TextShaper, TextStyle, TimerId,
    Waker, WidgetId, WindowId,
};
use xui_core::router::WidgetHost;
use xui_core::{Dip, Rect, Theme};

use super::canvas::metrics;
use super::scene::Node;
use super::{D2dBackend, Shared, window};
use crate::sys::gfx::TextParams;
use crate::sys::{clipboard, settings, window as sys};

impl Backend for D2dBackend {
    fn init(&self) {
        sys::init();
    }

    fn run(&self) -> i32 {
        sys::run_loop()
    }

    fn run_with(&self, window: WindowId, on_ready: &mut dyn FnMut()) -> i32 {
        on_ready();
        if let Some(hwnd) = self.shared.hwnd_of(window) {
            sys::show(hwnd);
        }
        self.run()
    }

    fn quit(&self, code: i32) {
        self.shared.exit_code.set(code);
        sys::post_quit(code);
    }

    fn wake(&self, window: WindowId) {
        if let Some(hwnd) = self.shared.hwnd_of(window) {
            sys::post_wake(hwnd);
        }
    }

    fn waker(&self, window: WindowId) -> Waker {
        match self.shared.hwnd_of(window) {
            Some(hwnd) => Box::new(move || sys::post_wake(hwnd)),
            None => Box::new(|| {}),
        }
    }

    fn set_event_sink(&self, window: WindowId, sink: Rc<dyn WidgetHost>) {
        if let Some(state) = self.shared.windows.borrow_mut().get_mut(&window.raw()) {
            state.sink = Some(sink);
        }
    }

    fn open_window(&self, spec: &PlatformSpec) -> Result<WindowId> {
        self.shared.open(spec)
    }

    fn close_window(&self, window: WindowId) {
        self.shared.close(window);
    }

    fn set_window_title(&self, window: WindowId, title: &str) {
        if let Some(hwnd) = self.shared.hwnd_of(window) {
            sys::set_title(hwnd, title);
        }
    }

    fn set_window_enabled(&self, window: WindowId, enabled: bool) {
        if let Some(hwnd) = self.shared.hwnd_of(window) {
            sys::set_enabled(hwnd, enabled);
        }
    }

    fn minimize(&self, window: WindowId) {
        if let Some(hwnd) = self.shared.hwnd_of(window) {
            sys::minimize(hwnd);
        }
    }

    fn toggle_maximize(&self, window: WindowId) {
        if let Some(hwnd) = self.shared.hwnd_of(window) {
            sys::toggle_maximize(hwnd);
        }
    }

    fn is_maximized(&self, window: WindowId) -> bool {
        self.shared.hwnd_of(window).is_some_and(sys::is_maximized)
    }

    fn caption_inset(&self, _window: WindowId) -> Dip {
        Dip(0.0)
    }

    fn create(&self, parent: ParentRef, spec: &NodeSpec) -> Result<WidgetId> {
        let shared = &self.shared;
        let window = match parent {
            ParentRef::Window(window) => window,
            ParentRef::Widget(widget) => shared
                .window_of(widget)
                .ok_or(BackendError::CreateFailed("parent node"))?,
        };
        if !shared.windows.borrow().contains_key(&window.raw()) {
            return Err(BackendError::CreateFailed("window"));
        }
        let id = WidgetId::from_raw(Shared::allocate(&shared.next_widget));
        let mut node = Node::new(window, parent, spec.bounds);
        node.visible = spec.visible;
        node.enabled = spec.enabled;
        node.text = spec.text.clone();
        node.popup = spec.popup;
        node.tab_stop = spec.tab_stop;
        node.wants_tab = spec.wants_tab;
        node.contained = spec.contained;
        shared.scene.borrow_mut().insert(id, node);
        Ok(id)
    }

    fn destroy(&self, id: WidgetId) {
        let Some(window) = self.shared.window_of(id) else {
            return;
        };
        self.shared.invalidate(id);
        let removed = self.shared.scene.borrow_mut().remove(id);
        for gone in removed {
            self.shared.forget(window, gone);
        }
    }

    fn apply_moves(&self, window: WindowId, moves: &[(WidgetId, Rect)]) {
        let mut resized = Vec::new();
        for (id, rect) in moves {
            let before = self.shared.scene.borrow().absolute_bounds(*id);
            let previous = {
                let mut scene = self.shared.scene.borrow_mut();
                scene
                    .get_mut(*id)
                    .map(|node| std::mem::replace(&mut node.bounds, *rect))
            };
            let Some(previous) = previous else {
                continue;
            };
            if previous == *rect {
                continue;
            }
            if let Some(before) = before {
                self.shared.invalidate_area(window, before);
            }
            self.shared.invalidate(*id);
            if previous.width() != rect.width() || previous.height() != rect.height() {
                resized.push((*id, *rect));
            }
        }
        // There is no child window to fire `WM_SIZE`, so a resized widget is
        // told directly or its cached layout goes stale.
        for (id, rect) in resized {
            let event = Event::Resize {
                width: rect.width(),
                height: rect.height(),
            };
            self.shared.deliver(window, id, &event);
        }
    }

    fn set_visible(&self, id: WidgetId, visible: bool) {
        self.with_node(id, |node| node.visible = visible);
    }

    fn set_enabled(&self, id: WidgetId, enabled: bool) {
        self.with_node(id, |node| node.enabled = enabled);
    }

    fn raise(&self, id: WidgetId) {
        if self.shared.scene.borrow_mut().raise(id) {
            self.shared.invalidate(id);
        }
    }

    fn set_cursor(&self, id: WidgetId, cursor: Cursor) {
        if let Some(node) = self.shared.scene.borrow_mut().get_mut(id) {
            node.cursor = cursor;
        }
    }

    fn set_clip(&self, id: WidgetId, rect: Option<Rect>) {
        self.with_node(id, |node| node.clip = rect);
    }

    fn set_capture(&self, id: WidgetId) {
        self.shared.captured.set(Some(id));
        if let Some(hwnd) = self
            .shared
            .window_of(id)
            .and_then(|w| self.shared.hwnd_of(w))
        {
            sys::capture(hwnd);
        }
    }

    fn release_capture(&self) {
        let previous = self.shared.captured.take();
        sys::release_capture();
        if let Some(id) = previous
            && let Some(window) = self.shared.window_of(id)
        {
            self.shared.deliver(window, id, &Event::CaptureChanged);
        }
    }

    fn focus(&self, id: WidgetId) {
        if let Some(hwnd) = self
            .shared
            .window_of(id)
            .and_then(|w| self.shared.hwnd_of(w))
        {
            sys::focus(hwnd);
        }
        self.shared.focus_node(id);
    }

    fn focus_holder(&self, window: WindowId) -> Option<WidgetId> {
        self.shared.focus_holder(window)
    }

    fn set_text(&self, id: WidgetId, text: &str) {
        self.with_node(id, |node| node.text = text.to_owned());
    }

    fn text(&self, id: WidgetId) -> String {
        self.shared
            .scene
            .borrow()
            .get(id)
            .map(|node| node.text.clone())
            .unwrap_or_default()
    }

    fn clipboard_text(&self) -> Option<String> {
        clipboard::get()
    }

    fn set_clipboard_text(&self, text: &str) {
        clipboard::set(text);
    }

    fn bounds(&self, id: WidgetId) -> Rect {
        self.shared
            .scene
            .borrow()
            .get(id)
            .map_or(Rect::default(), |node| node.bounds)
    }

    fn invalidate(&self, id: WidgetId) {
        self.shared.invalidate(id);
    }

    fn invalidate_rect(&self, id: WidgetId, rect: Rect) {
        let origin = self.shared.scene.borrow().absolute_bounds(id);
        if let (Some(window), Some(origin)) = (self.shared.window_of(id), origin) {
            self.shared
                .invalidate_area(window, rect.offset(origin.left, origin.top));
        }
    }

    fn set_painter(&self, id: WidgetId, painter: Painter) {
        self.with_node(id, |node| node.painter = Some(painter));
    }

    fn measure_text(&self, text: &str, style: &TextStyle, dpi: u32) -> TextMetrics {
        metrics(
            text,
            &TextParams {
                family: style.family.as_deref(),
                size_px: style.size.to_px(dpi).value() as f32,
                weight: style.weight.value(),
                italic: style.italic,
                align: style.align,
                valign: style.valign,
                wrap: false,
            },
        )
    }

    fn text_shaper(&self) -> Box<dyn TextShaper> {
        match &self.shared.shaper {
            Some(shaper) => Box::new(shaper.clone()),
            None => Box::new(NoShaper),
        }
    }

    fn layout_text(
        &self,
        text: &str,
        spec: &FontSpec,
        max_width: f32,
        dpi: u32,
    ) -> Box<dyn TextLayout> {
        self.text_shaper().layout(text, spec, max_width, dpi)
    }

    fn dpi(&self, window: WindowId) -> u32 {
        self.shared.dpi_of(window)
    }

    fn client_rect(&self, window: WindowId) -> Rect {
        self.shared
            .windows
            .borrow()
            .get(&window.raw())
            .map_or(Rect::default(), |state| {
                Rect::new(0, 0, state.size.0, state.size.1)
            })
    }

    fn system_theme(&self) -> Option<Theme> {
        Some(window::system_theme())
    }

    fn prefers_reduced_motion(&self) -> bool {
        settings::reduced_motion()
    }

    fn set_theme(&self, window: WindowId, theme: &Theme) {
        let hwnd = {
            let mut windows = self.shared.windows.borrow_mut();
            let Some(state) = windows.get_mut(&window.raw()) else {
                return;
            };
            state.theme = *theme;
            state.hwnd
        };
        sys::set_dark_title_bar(hwnd, theme.is_dark);
        self.shared.invalidate_window(window);
    }

    fn set_timer(&self, window: WindowId, millis: u32) -> TimerId {
        let Some(hwnd) = self.shared.hwnd_of(window) else {
            return TimerId(0);
        };
        let id = self.shared.next_timer.get();
        self.shared.next_timer.set(id + 1);
        if !sys::set_timer(hwnd, id, millis) {
            return TimerId(0);
        }
        self.shared.timers.borrow_mut().insert(id, window.raw());
        TimerId(id)
    }

    fn kill_timer(&self, window: WindowId, id: TimerId) {
        if let Some(hwnd) = self.shared.hwnd_of(window) {
            sys::kill_timer(hwnd, id.0);
        }
        self.shared.timers.borrow_mut().remove(&id.0);
    }

    fn supports(&self, _kind: NodeKind) -> ImplKind {
        ImplKind::Painted
    }
}

impl D2dBackend {
    /// Changes a node and repaints it.
    fn with_node(&self, id: WidgetId, change: impl FnOnce(&mut Node)) {
        if let Some(node) = self.shared.scene.borrow_mut().get_mut(id) {
            change(node);
        }
        self.shared.invalidate(id);
    }
}

/// The shaper when DirectWrite is unavailable: it shapes nothing.
struct NoShaper;

impl TextShaper for NoShaper {
    fn layout(&self, _: &str, _: &FontSpec, _: f32, _: u32) -> Box<dyn TextLayout> {
        Box::new(crate::sys::gfx::NoLayout)
    }
}
