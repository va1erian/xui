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

use crate::gl::GlWidget;
use crate::text_layout::CosmicShaper;
use crate::{RgbaImage, Surface};

mod backend;
#[cfg(test)]
mod capture_tests;
mod geometry;
#[cfg(test)]
mod tests;

use crate::backend::geometry::{
    absolute_bounds, ancestor_clip, effectively_visible, hit_bounds, intersect,
};
use geometry::translate;

/// The default dots-per-inch a surface is rendered at.
const DEFAULT_DPI: u32 = 96;

struct OffscreenWindow {
    surface: Surface,
    sink: Option<Rc<dyn WidgetHost>>,
    theme: Theme,
    dpi: u32,
    width: i32,
    height: i32,
    /// Window-level GL content, painted through its software fallback (the
    /// offscreen backend never has a GL context).
    gl: Option<Rc<dyn GlWidget>>,
    /// Per-node GL content, keyed by the node it fills, also painted through its
    /// software fallback.
    gl_nodes: HashMap<u64, Rc<dyn GlWidget>>,
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

/// One node to draw in creation order: its painter (if any) and whether it also
/// carries GL fallback content.
struct Draw {
    id: WidgetId,
    bounds: Rect,
    clip: Option<Rect>,
    painter: Option<Painter>,
    gl: bool,
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
                theme: Theme::light(),
                dpi,
                width: width as i32,
                height: height as i32,
                gl: None,
                gl_nodes: HashMap::new(),
            },
        );
        Ok(id)
    }

    /// Renders `window`'s visible nodes, in creation order, into an image.
    pub fn render(&self, window: WindowId) -> Option<RgbaImage> {
        let mut windows = self.windows.borrow_mut();
        let entry = windows.get_mut(&window.raw())?;
        entry.surface.fill(entry.theme.background);
        let dpi = entry.dpi;
        let theme = entry.theme;
        // GL content is one painter among many, exactly as in the windowed
        // backend; the offscreen backend has no GPU, so it paints the widget's
        // software fallback: window-level content as the base layer, node-level
        // content at its node's bounds.
        if let Some(widget) = entry.gl.clone() {
            let bounds = Rect::new(0, 0, entry.width, entry.height);
            entry
                .surface
                .with_canvas_at(bounds, dpi, |canvas| widget.paint(canvas, bounds, &theme));
        }
        let gl_nodes: Vec<(u64, Rc<dyn GlWidget>)> = entry
            .gl_nodes
            .iter()
            .map(|(raw, widget)| (*raw, Rc::clone(widget)))
            .collect();
        let nodes = self.nodes.borrow();
        let paints: Vec<Draw> = nodes
            .iter()
            .filter(|(id, node)| node.window == window && effectively_visible(&nodes, *id))
            .filter_map(|(id, node)| {
                let painter = node.painter.clone();
                let gl = gl_nodes.iter().any(|(raw, _)| *raw == id.raw());
                if painter.is_none() && !gl {
                    return None;
                }
                let bounds = absolute_bounds(&nodes, *id)?;
                let clip = ancestor_clip(&nodes, *id);
                if let Some(clip) = clip
                    && intersect(bounds, clip).is_empty()
                {
                    return None;
                }
                Some(Draw {
                    id: *id,
                    bounds,
                    clip,
                    painter,
                    gl,
                })
            })
            .collect();
        drop(nodes);
        for draw in paints {
            if let Some(painter) = draw.painter {
                entry.surface.with_canvas_at(draw.bounds, dpi, |canvas| {
                    if let Some(clip) = draw.clip {
                        canvas.push_clip(clip);
                    }
                    painter(canvas);
                });
            }
            if draw.gl
                && let Some((_, widget)) = gl_nodes.iter().find(|(raw, _)| *raw == draw.id.raw())
            {
                entry.surface.with_canvas_at(draw.bounds, dpi, |canvas| {
                    if let Some(clip) = draw.clip {
                        canvas.push_clip(clip);
                    }
                    widget.paint(canvas, draw.bounds, &theme);
                });
            }
        }
        Some(entry.surface.to_image())
    }

    /// Installs `widget` as `window`'s GL content. The offscreen backend has no
    /// GPU, so [`OffscreenBackend::render`] paints the widget's software
    /// fallback; this mirrors the windowed backend's seam and lets a headless
    /// test exercise the fallback.
    pub fn set_gl_content<W: GlWidget + 'static>(&self, window: WindowId, widget: W) {
        if let Some(entry) = self.windows.borrow_mut().get_mut(&window.raw()) {
            entry.gl = Some(Rc::new(widget));
        }
    }

    /// Removes `window`'s GL content.
    pub fn clear_gl_content(&self, window: WindowId) {
        if let Some(entry) = self.windows.borrow_mut().get_mut(&window.raw()) {
            entry.gl = None;
        }
    }

    /// Installs `widget` as the GL content of the node `id`, painted through its
    /// software fallback at the node's bounds. Mirrors the windowed backend's
    /// per-node seam and lets a headless test exercise the fallback in a pane.
    pub fn set_gl_content_on<W: GlWidget + 'static>(&self, id: WidgetId, widget: W) {
        let window = self
            .nodes
            .borrow()
            .iter()
            .find(|(node_id, _)| *node_id == id)
            .map(|(_, node)| node.window);
        if let Some(window) = window
            && let Some(entry) = self.windows.borrow_mut().get_mut(&window.raw())
        {
            entry.gl_nodes.insert(id.raw(), Rc::new(widget));
        }
    }

    /// Removes the GL content of the node `id`.
    pub fn clear_gl_content_on(&self, id: WidgetId) {
        let window = self
            .nodes
            .borrow()
            .iter()
            .find(|(node_id, _)| *node_id == id)
            .map(|(_, node)| node.window);
        if let Some(window) = window
            && let Some(entry) = self.windows.borrow_mut().get_mut(&window.raw())
        {
            entry.gl_nodes.remove(&id.raw());
        }
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
                nodes.iter().rev().find_map(|(id, node)| {
                    if node.window != window {
                        return None;
                    }
                    let abs = hit_bounds(&nodes, *id, x, y)?;
                    Some((*id, x - abs.left, y - abs.top))
                })
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
