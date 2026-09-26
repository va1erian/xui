#![forbid(unsafe_code)]

//! An offscreen [`xui_core::backend::Backend`] that composites the portable
//! widgets into a software [`Surface`] instead of a window.
//!
//! It runs on every platform with no UI toolkit, so the same widget code that
//! the Win32 backend hosts can be rendered and snapshot-tested here (including
//! on headless CI runners). The windowing shell builds on the same surface.

use std::cell::{OnceCell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use xui_core::backend::{
    Canvas as _, Event, Painter, ParentRef, PlatformSpec, Result as BackendResult, WidgetId,
    WindowId,
};
use xui_core::router::WidgetHost;
use xui_core::{Rect, Theme};

use crate::text_layout::CosmicShaper;
use crate::{RgbaImage, Surface};

mod backend;
mod geometry;
#[cfg(test)]
mod tests;

use geometry::{absolute_bounds, ancestor_clip, intersect, translate};

/// The default dots-per-inch a surface is rendered at.
const DEFAULT_DPI: u32 = 96;

struct OffscreenWindow {
    surface: Surface,
    sink: Option<Rc<dyn WidgetHost>>,
    background: xui_core::Color,
    dpi: u32,
    width: i32,
    height: i32,
}

struct Node {
    window: WindowId,
    parent: ParentRef,
    bounds: Rect,
    visible: bool,
    enabled: bool,
    text: String,
    painter: Option<Painter>,
    /// A clip on this node's descendants, in the node's own coordinates.
    clip: Option<Rect>,
}

/// A backend that renders into a software surface.
pub struct OffscreenBackend {
    windows: RefCell<HashMap<u64, OffscreenWindow>>,
    nodes: RefCell<Vec<(WidgetId, Node)>>,
    next_window: std::cell::Cell<u64>,
    next_widget: std::cell::Cell<u64>,
    /// The node the pointer is captured by, if any.
    captured: RefCell<Option<WidgetId>>,
    text: OnceCell<CosmicShaper>,
}

impl Default for OffscreenBackend {
    fn default() -> OffscreenBackend {
        OffscreenBackend::new()
    }
}

impl OffscreenBackend {
    /// A backend with no windows.
    pub fn new() -> OffscreenBackend {
        OffscreenBackend {
            windows: RefCell::new(HashMap::new()),
            nodes: RefCell::new(Vec::new()),
            next_window: std::cell::Cell::new(1),
            next_widget: std::cell::Cell::new(1),
            captured: RefCell::new(None),
            text: OnceCell::new(),
        }
    }

    fn allocate(cell: &std::cell::Cell<u64>) -> u64 {
        let id = cell.get();
        cell.set(id + 1);
        id
    }

    /// Opens a window rendered at `dpi`, so a test can exercise high-DPI
    /// layouts. [`xui_core::backend::Backend::open_window`] uses
    /// [`DEFAULT_DPI`].
    pub fn open_window_at(&self, spec: &PlatformSpec, dpi: u32) -> BackendResult<WindowId> {
        let id = WindowId::from_raw(Self::allocate(&self.next_window));
        let width = spec.width.to_px(dpi).value().max(1) as u32;
        let height = spec.height.to_px(dpi).value().max(1) as u32;
        self.windows.borrow_mut().insert(
            id.raw(),
            OffscreenWindow {
                surface: Surface::new(width, height),
                sink: None,
                background: Theme::light().background,
                dpi,
                width: width as i32,
                height: height as i32,
            },
        );
        Ok(id)
    }

    /// Renders `window`'s visible nodes, in creation order, into an image.
    pub fn render(&self, window: WindowId) -> Option<RgbaImage> {
        let mut windows = self.windows.borrow_mut();
        let entry = windows.get_mut(&window.raw())?;
        entry.surface.fill(entry.background);
        let dpi = entry.dpi;
        let nodes = self.nodes.borrow();
        let paints: Vec<(Rect, Option<Rect>, Painter)> = nodes
            .iter()
            .filter(|(_, node)| node.window == window && node.visible)
            .filter_map(|(id, node)| {
                let painter = node.painter.clone()?;
                let clip = ancestor_clip(&nodes, *id);
                if let Some(clip) = clip
                    && intersect(node.bounds, clip).is_empty()
                {
                    return None;
                }
                Some((node.bounds, clip, painter))
            })
            .collect();
        drop(nodes);
        for (bounds, clip, painter) in paints {
            entry.surface.with_canvas_at(bounds, dpi, |canvas| {
                if let Some(clip) = clip {
                    canvas.push_clip(clip);
                }
                painter(canvas);
            });
        }
        Some(entry.surface.to_image())
    }

    /// Delivers `event` to the topmost node under its position, as a window
    /// system would, returning whether it was consumed. A captured node
    /// receives every pointer move and release, even outside its bounds.
    pub fn inject(&self, window: WindowId, event: Event) -> bool {
        let Some((x, y)) = event.position() else {
            return false;
        };
        let captured = *self.captured.borrow();
        let target = match captured {
            Some(id) => {
                let nodes = self.nodes.borrow();
                let abs = absolute_bounds(&nodes, id);
                abs.map(|abs| (id, x - abs.left, y - abs.top))
            }
            None => {
                let nodes = self.nodes.borrow();
                nodes
                    .iter()
                    .rev()
                    .find(|(_, node)| {
                        node.window == window
                            && node.visible
                            && node.enabled
                            && node.bounds.contains(xui_core::Point::new(x, y))
                    })
                    .map(|(id, node)| (*id, x - node.bounds.left, y - node.bounds.top))
            }
        };
        let Some((target, lx, ly)) = target else {
            return false;
        };
        let sink = self
            .windows
            .borrow()
            .get(&window.raw())
            .and_then(|entry| entry.sink.clone());
        let event = translate(event, lx, ly);
        sink.is_some_and(|sink| sink.deliver(target, &event))
    }

    /// The dots-per-inch `window` is rendered at.
    pub fn dpi_of(&self, window: WindowId) -> u32 {
        self.windows
            .borrow()
            .get(&window.raw())
            .map_or(DEFAULT_DPI, |entry| entry.dpi)
    }

    fn with_node<R>(&self, id: WidgetId, f: impl FnOnce(&mut Node) -> R) -> Option<R> {
        self.nodes
            .borrow_mut()
            .iter_mut()
            .find(|(node_id, _)| *node_id == id)
            .map(|(_, node)| f(node))
    }
}
