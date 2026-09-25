#![forbid(unsafe_code)]

//! A test-only backend that creates no windows: it records what the front layer
//! asks for and lets a test inject events. It proves the [`Backend`] contract
//! is implementable without a platform and gives the widget layer a way to be
//! tested headlessly.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use super::canvas::{TextMetrics, TextStyle};
use super::event::{Event, TimerId};
use super::ids::{WidgetId, WindowId};
use super::node::{ImplKind, NodeKind, NodeSpec, ParentRef};
use super::{Backend, BackendError, Painter, PlatformSpec, Result};
use crate::geometry::Rect;
use crate::router::WidgetHost;
use crate::theme::Theme;

mod canvas;
#[cfg(test)]
mod tests;

pub(crate) use canvas::DrawOp;
use canvas::RecordingCanvas;
/// A node the headless backend recorded.
#[derive(Clone)]
struct Node {
    kind: NodeKind,
    parent: ParentRef,
    bounds: Rect,
    text: String,
    visible: bool,
    enabled: bool,
    painter: Option<Painter>,
    ops: Vec<DrawOp>,
}

/// A window the headless backend recorded.
struct SinkWindow {
    title: String,
    client: Rect,
    dpi: u32,
    sink: Option<Rc<dyn WidgetHost>>,
    theme: Theme,
    wakes: u32,
}

/// What the headless backend recorded so far.
pub struct HeadlessBackend {
    state: RefCell<State>,
}

struct State {
    next_window: u64,
    next_widget: u64,
    next_timer: u64,
    quit: bool,
    focused: Option<WidgetId>,
    moves: u32,
    invalidations: u32,
    windows: HashMap<u64, SinkWindow>,
    nodes: HashMap<u64, Node>,
}

impl HeadlessBackend {
    /// An empty backend.
    pub fn new() -> HeadlessBackend {
        HeadlessBackend {
            state: RefCell::new(State {
                next_window: 1,
                next_widget: 1,
                next_timer: 1,
                quit: false,
                focused: None,
                moves: 0,
                invalidations: 0,
                windows: HashMap::new(),
                nodes: HashMap::new(),
            }),
        }
    }

    /// Whether the loop was asked to quit.
    pub fn quit_requested(&self) -> bool {
        self.state.borrow().quit
    }

    /// The node a widget id names, if any.
    pub fn node(&self, id: WidgetId) -> Option<(NodeKind, Rect, String, bool, bool)> {
        self.state.borrow().nodes.get(&id.raw()).map(|node| {
            (
                node.kind,
                node.bounds,
                node.text.clone(),
                node.visible,
                node.enabled,
            )
        })
    }

    /// The drawing commands `id` recorded during the last [`render`].
    ///
    /// [`render`]: HeadlessBackend::render
    pub fn ops(&self, id: WidgetId) -> Vec<DrawOp> {
        self.state
            .borrow()
            .nodes
            .get(&id.raw())
            .map(|node| node.ops.clone())
            .unwrap_or_default()
    }

    /// Runs `id`'s registered painter as a backend would on paint, recording
    /// the drawing commands it makes.
    pub fn render(&self, id: WidgetId) {
        let (bounds, painter) = match self.state.borrow().nodes.get(&id.raw()) {
            Some(node) => (node.bounds, node.painter.clone()),
            None => return,
        };
        let Some(painter) = painter else {
            return;
        };
        let mut canvas = RecordingCanvas::new(bounds, 96);
        painter(&mut canvas);
        if let Some(node) = self.state.borrow_mut().nodes.get_mut(&id.raw()) {
            node.ops = canvas.ops;
        }
    }

    /// Whether a node with `id` still exists.
    pub fn has_node(&self, id: WidgetId) -> bool {
        self.state.borrow().nodes.contains_key(&id.raw())
    }

    /// How many batched move calls were made.
    pub fn move_calls(&self) -> u32 {
        self.state.borrow().moves
    }

    /// How many invalidations were requested.
    pub fn invalidations(&self) -> u32 {
        self.state.borrow().invalidations
    }

    /// Whether `widget` currently has the focus.
    pub fn focused(&self) -> Option<WidgetId> {
        self.state.borrow().focused
    }

    /// Delivers `event` to `target` through `window`'s sink, as a backend
    /// would after decoding native input.
    pub fn inject(&self, window: WindowId, target: WidgetId, event: Event) -> bool {
        let sink = self
            .state
            .borrow()
            .windows
            .get(&window.raw())
            .and_then(|w| w.sink.clone());
        match sink {
            Some(sink) => sink.deliver(target, &event),
            None => false,
        }
    }

    /// The title `window` was opened with.
    pub fn window_title(&self, window: WindowId) -> Option<String> {
        self.state
            .borrow()
            .windows
            .get(&window.raw())
            .map(|w| w.title.clone())
    }

    /// The theme last applied to `window`.
    pub fn window_theme(&self, window: WindowId) -> Option<Theme> {
        self.state
            .borrow()
            .windows
            .get(&window.raw())
            .map(|w| w.theme)
    }

    /// How many times `window` was woken.
    pub fn wakes(&self, window: WindowId) -> u32 {
        self.state
            .borrow()
            .windows
            .get(&window.raw())
            .map_or(0, |w| w.wakes)
    }

    fn size(spec: &PlatformSpec) -> Rect {
        Rect::new(
            0,
            0,
            spec.width.to_px(96).value(),
            spec.height.to_px(96).value(),
        )
    }
}

/// Removes every node whose parent chain no longer exists: a window that was
/// closed, or a node that was destroyed.
fn remove_orphans(state: &mut State) {
    loop {
        let doomed: Vec<u64> = state
            .nodes
            .iter()
            .filter(|(_, node)| match node.parent {
                ParentRef::Window(window) => !state.windows.contains_key(&window.raw()),
                ParentRef::Widget(parent) => !state.nodes.contains_key(&parent.raw()),
            })
            .map(|(id, _)| *id)
            .collect();
        if doomed.is_empty() {
            break;
        }
        for id in doomed {
            state.nodes.remove(&id);
        }
    }
}

impl Default for HeadlessBackend {
    fn default() -> HeadlessBackend {
        HeadlessBackend::new()
    }
}

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
        }
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
            },
        );
        Ok(WindowId::from_raw(id))
    }

    fn close_window(&self, window: WindowId) {
        let mut state = self.state.borrow_mut();
        state.windows.remove(&window.raw());
        remove_orphans(&mut state);
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
            },
        );
        Ok(WidgetId::from_raw(id))
    }

    fn destroy(&self, id: WidgetId) {
        let mut state = self.state.borrow_mut();
        state.nodes.remove(&id.raw());
        remove_orphans(&mut state);
    }

    fn apply_moves(&self, _window: WindowId, moves: &[(WidgetId, Rect)]) {
        let mut state = self.state.borrow_mut();
        state.moves += 1;
        for (id, rect) in moves {
            if let Some(node) = state.nodes.get_mut(&id.raw()) {
                node.bounds = *rect;
            }
        }
    }

    fn set_visible(&self, id: WidgetId, visible: bool) {
        if let Some(node) = self.state.borrow_mut().nodes.get_mut(&id.raw()) {
            node.visible = visible;
        }
    }

    fn set_enabled(&self, id: WidgetId, enabled: bool) {
        if let Some(node) = self.state.borrow_mut().nodes.get_mut(&id.raw()) {
            node.enabled = enabled;
        }
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

    fn measure_text(&self, text: &str, style: &TextStyle, dpi: u32) -> TextMetrics {
        let height = (style.size.to_px(dpi).value() as f32 * 1.25).round() as i32;
        let width = (text.chars().count() as f32 * style.size.to_px(dpi).value() as f32 * 0.5)
            .round() as i32;
        TextMetrics {
            width,
            height,
            ascent: height * 3 / 4,
            descent: height / 4,
        }
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
