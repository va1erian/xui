#![forbid(unsafe_code)]

//! The [`Backend`] contract over the headless recorder.

use std::rc::Rc;

use super::text::HeadlessShaper;
use super::{HeadlessBackend, Node, SinkWindow, remove_orphans};
use crate::backend::{
    Backend, BackendError, Event, FileDialogOutcome, FileDialogRequest, FontSpec, ImplKind,
    NodeKind, NodeSpec, Painter, ParentRef, PlatformSpec, Result, TextLayout, TextMetrics,
    TextShaper, TextStyle, TimerId, Waker, WidgetId, WindowId,
};
use crate::geometry::Rect;
use crate::image::Image;
use crate::router::WidgetHost;
use crate::theme::Theme;

impl Backend for HeadlessBackend {
    fn run(&self) -> i32 {
        0
    }

    fn quit(&self, _code: i32) {
        self.state.borrow_mut().quit = true;
    }

    fn wake(&self, window: WindowId) {
        if let Some(w) = self.state.borrow_mut().windows.get_mut(&window.raw()) {
            w.wakes += 1;
            w.pending_wakes += 1;
        }
    }

    fn waker(&self, _window: WindowId) -> Waker {
        // The headless backend has no loop to wake; a test drives `Wake`
        // itself after sending through a proxy.
        Box::new(|| {})
    }

    fn set_event_sink(&self, window: WindowId, sink: Rc<dyn WidgetHost>) {
        if let Some(w) = self.state.borrow_mut().windows.get_mut(&window.raw()) {
            w.sink = Some(sink);
        }
    }

    fn open_window(&self, spec: &PlatformSpec) -> Result<WindowId> {
        let mut state = self.state.borrow_mut();
        let id = state.next_window;
        state.next_window += 1;
        state.windows.insert(
            id,
            SinkWindow {
                title: spec.title.clone(),
                client: Self::size(spec),
                dpi: 96,
                sink: None,
                theme: Theme::light(),
                wakes: 0,
                pending_wakes: 0,
                enabled: true,
            },
        );
        Ok(WindowId::from_raw(id))
    }

    fn close_window(&self, window: WindowId) {
        let mut state = self.state.borrow_mut();
        state.windows.remove(&window.raw());
        remove_orphans(&mut state);
    }

    fn set_window_title(&self, window: WindowId, title: &str) {
        if let Some(w) = self.state.borrow_mut().windows.get_mut(&window.raw()) {
            w.title = title.to_string();
        }
    }

    fn set_window_enabled(&self, window: WindowId, enabled: bool) {
        if let Some(w) = self.state.borrow_mut().windows.get_mut(&window.raw()) {
            w.enabled = enabled;
        }
    }

    fn capture(&self, window: WindowId) -> Result<Image> {
        let state = self.state.borrow();
        let Some(w) = state.windows.get(&window.raw()) else {
            return Err(BackendError::Other("no such window".into()));
        };
        let (width, height) = (
            w.client.width().max(0) as u32,
            w.client.height().max(0) as u32,
        );
        let color = w.theme.background;
        let pixel = [color.r, color.g, color.b, 255];
        let pixels = pixel.repeat(width as usize * height as usize);
        Image::from_rgba(width, height, pixels).map_err(|e| BackendError::Other(e.to_string()))
    }

    fn run_modal(&self, _window: WindowId) -> Result<()> {
        Ok(())
    }

    fn file_dialog(&self, _window: WindowId, request: &FileDialogRequest) -> FileDialogOutcome {
        let mut state = self.state.borrow_mut();
        state.file_dialog_request = Some(request.clone());
        state.file_dialog.clone()
    }

    fn create(&self, parent: ParentRef, spec: &NodeSpec) -> Result<WidgetId> {
        let mut state = self.state.borrow_mut();
        let parent_exists = match parent {
            ParentRef::Window(w) => state.windows.contains_key(&w.raw()),
            ParentRef::Widget(w) => state.nodes.contains_key(&w.raw()),
        };
        if !parent_exists {
            return Err(BackendError::CreateFailed("parent node"));
        }
        let id = state.next_widget;
        state.next_widget += 1;
        state.nodes.insert(
            id,
            Node {
                kind: spec.kind,
                parent,
                bounds: spec.bounds,
                text: spec.text.clone(),
                visible: spec.visible,
                enabled: spec.enabled,
                painter: None,
                ops: Vec::new(),
                clip: None,
                popup: spec.popup,
            },
        );
        Ok(WidgetId::from_raw(id))
    }

    fn destroy(&self, id: WidgetId) {
        let mut state = self.state.borrow_mut();
        state.nodes.remove(&id.raw());
        remove_orphans(&mut state);
    }

    fn apply_moves(&self, window: WindowId, moves: &[(WidgetId, Rect)]) {
        let mut resized = Vec::new();
        {
            let mut state = self.state.borrow_mut();
            state.moves += 1;
            for (id, rect) in moves {
                if let Some(node) = state.nodes.get_mut(&id.raw()) {
                    if node.bounds.size() != rect.size() {
                        resized.push((*id, *rect));
                    }
                    node.bounds = *rect;
                }
            }
        }
        // As the canvas backend does: a moved node has no native size
        // notification, so a resized one is told directly.
        for (id, rect) in resized {
            self.inject(
                window,
                id,
                Event::Resize {
                    width: rect.width(),
                    height: rect.height(),
                },
            );
        }
    }

    fn set_visible(&self, id: WidgetId, visible: bool) {
        let shown = {
            let mut state = self.state.borrow_mut();
            match state.nodes.get_mut(&id.raw()) {
                Some(node) => {
                    let was = node.visible;
                    node.visible = visible;
                    visible && !was
                }
                None => false,
            }
        };
        // A native show paints synchronously before the window is composed
        // (the Win32 backend cloaks, paints and uncloaks), so model it: a
        // test then sees the same first frame a platform would compose.
        if shown {
            self.render(id);
        }
    }

    fn set_enabled(&self, id: WidgetId, enabled: bool) {
        if let Some(node) = self.state.borrow_mut().nodes.get_mut(&id.raw()) {
            node.enabled = enabled;
        }
    }

    fn set_clip(&self, id: WidgetId, rect: Option<Rect>) {
        if let Some(node) = self.state.borrow_mut().nodes.get_mut(&id.raw()) {
            node.clip = rect;
        }
    }

    fn set_capture(&self, id: WidgetId) {
        self.state.borrow_mut().captured = Some(id);
    }

    fn release_capture(&self) {
        self.state.borrow_mut().captured = None;
    }

    fn focus(&self, id: WidgetId) {
        self.state.borrow_mut().focused = Some(id);
    }

    fn set_text(&self, id: WidgetId, text: &str) {
        if let Some(node) = self.state.borrow_mut().nodes.get_mut(&id.raw()) {
            node.text = text.to_string();
        }
    }

    fn invalidate(&self, _id: WidgetId) {
        self.state.borrow_mut().invalidations += 1;
    }

    fn invalidate_rect(&self, _id: WidgetId, _rect: Rect) {
        self.state.borrow_mut().invalidations += 1;
    }

    fn set_painter(&self, id: WidgetId, painter: Painter) {
        if let Some(node) = self.state.borrow_mut().nodes.get_mut(&id.raw()) {
            node.painter = Some(painter);
        }
    }

    fn bounds(&self, id: WidgetId) -> Rect {
        self.state
            .borrow()
            .nodes
            .get(&id.raw())
            .map_or(Rect::default(), |node| node.bounds)
    }

    fn measure_text(&self, text: &str, style: &TextStyle, dpi: u32) -> TextMetrics {
        super::text::estimate_metrics(text, style, dpi)
    }

    fn text_shaper(&self) -> Box<dyn TextShaper> {
        Box::new(HeadlessShaper)
    }

    fn layout_text(
        &self,
        text: &str,
        spec: &FontSpec,
        max_width: f32,
        dpi: u32,
    ) -> Box<dyn TextLayout> {
        HeadlessShaper.layout(text, spec, max_width, dpi)
    }

    fn dpi(&self, window: WindowId) -> u32 {
        self.state
            .borrow()
            .windows
            .get(&window.raw())
            .map_or(96, |w| w.dpi)
    }

    fn client_rect(&self, window: WindowId) -> Rect {
        self.state
            .borrow()
            .windows
            .get(&window.raw())
            .map_or(Rect::default(), |w| w.client)
    }

    fn set_theme(&self, window: WindowId, theme: &Theme) {
        if let Some(w) = self.state.borrow_mut().windows.get_mut(&window.raw()) {
            w.theme = *theme;
        }
    }

    fn set_timer(&self, _window: WindowId, _millis: u32) -> TimerId {
        let mut state = self.state.borrow_mut();
        let id = state.next_timer;
        state.next_timer += 1;
        TimerId(id as usize)
    }

    fn kill_timer(&self, _window: WindowId, _id: TimerId) {}

    fn supports(&self, _kind: NodeKind) -> ImplKind {
        ImplKind::Painted
    }
}
