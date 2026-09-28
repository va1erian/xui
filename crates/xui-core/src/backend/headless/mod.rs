#![forbid(unsafe_code)]

//! A test-only backend that creates no windows: it records what the front layer
//! asks for and lets a test inject events. It proves the [`Backend`] contract
//! is implementable without a platform and gives the widget layer a way to be
//! tested headlessly.
//!
//! [`Backend`]: super::Backend

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use super::event::Event;
use super::ids::{WidgetId, WindowId};
use super::node::{NodeKind, ParentRef};
use super::{Painter, PlatformSpec};
use crate::geometry::Rect;
use crate::router::WidgetHost;
use crate::theme::Theme;

mod backend;
mod canvas;
#[cfg(test)]
mod tests;
mod text;

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
    /// The clip set with [`Backend::set_clip`], recorded for tests.
    ///
    /// [`Backend::set_clip`]: super::Backend::set_clip
    clip: Option<Rect>,
    /// Whether the node asked to be a transient popup surface.
    popup: bool,
}

/// A window the headless backend recorded.
struct SinkWindow {
    title: String,
    client: Rect,
    dpi: u32,
    sink: Option<Rc<dyn WidgetHost>>,
    theme: Theme,
    wakes: u32,
    /// Whether the window is enabled. A modal opener is disabled while its
    /// child runs.
    enabled: bool,
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
    captured: Option<WidgetId>,
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
                captured: None,
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

    /// The clip last set on `id` with [`Backend::set_clip`], if any.
    ///
    /// [`Backend::set_clip`]: super::Backend::set_clip
    pub fn clip(&self, id: WidgetId) -> Option<Rect> {
        self.state
            .borrow()
            .nodes
            .get(&id.raw())
            .and_then(|node| node.clip)
    }

    /// Whether `id` was created as a transient popup surface.
    pub fn is_popup(&self, id: WidgetId) -> bool {
        self.state
            .borrow()
            .nodes
            .get(&id.raw())
            .is_some_and(|node| node.popup)
    }

    /// The node the pointer is currently captured by, if any.
    pub fn captured(&self) -> Option<WidgetId> {
        self.state.borrow().captured
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

    /// Resizes `window`'s client area and delivers the window-level
    /// [`Event::Resize`], as a backend does after the platform resizes it.
    pub fn resize_window(&self, window: WindowId, width: i32, height: i32) {
        if let Some(w) = self.state.borrow_mut().windows.get_mut(&window.raw()) {
            w.client = Rect::new(0, 0, width, height);
        }
        self.inject(window, WidgetId::NONE, Event::Resize { width, height });
    }

    /// Changes `window`'s DPI and delivers the window-level
    /// [`Event::DpiChanged`].
    pub fn set_window_dpi(&self, window: WindowId, dpi: u32) {
        let client = {
            let mut state = self.state.borrow_mut();
            let Some(w) = state.windows.get_mut(&window.raw()) else {
                return;
            };
            w.dpi = dpi;
            w.client
        };
        self.inject(
            window,
            WidgetId::NONE,
            Event::DpiChanged {
                dpi,
                suggested: client,
            },
        );
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

    /// Whether `window` is currently enabled.
    pub fn window_enabled(&self, window: WindowId) -> bool {
        self.state
            .borrow()
            .windows
            .get(&window.raw())
            .is_some_and(|w| w.enabled)
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
