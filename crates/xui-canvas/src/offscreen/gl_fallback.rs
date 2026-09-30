#![forbid(unsafe_code)]

//! GL content installed on an offscreen window, painted through its software
//! fallback. The offscreen backend never has a GL context, so a `GlWidget`
//! installed here is the same seam the windowed backend exposes, exercised
//! headlessly.
//!
//! With the `winit-backend` feature off there is no `GlWidget` seam at all, so
//! this is a zero-sized placeholder whose methods do nothing; the offscreen
//! painter then compiles without the windowed modules.

#[cfg(feature = "winit-backend")]
use std::collections::HashMap;
#[cfg(feature = "winit-backend")]
use std::rc::Rc;

#[cfg(feature = "winit-backend")]
use xui_core::backend::Canvas as _;
use xui_core::backend::WidgetId;
use xui_core::{Rect, Theme};

use crate::Surface;
#[cfg(feature = "winit-backend")]
use crate::gl::GlWidget;

#[cfg(feature = "winit-backend")]
pub(super) struct GlContent {
    width: i32,
    height: i32,
    window: Option<Rc<dyn GlWidget>>,
    nodes: HashMap<u64, Rc<dyn GlWidget>>,
}

#[cfg(feature = "winit-backend")]
impl GlContent {
    /// Empty GL content for a window of `width` x `height` logical pixels.
    pub(super) fn new(width: i32, height: i32) -> GlContent {
        GlContent {
            width,
            height,
            window: None,
            nodes: HashMap::new(),
        }
    }

    /// Installs `widget` as the window-level content.
    pub(super) fn set_window<W: GlWidget + 'static>(&mut self, widget: W) {
        self.window = Some(Rc::new(widget));
    }

    /// Removes the window-level content.
    pub(super) fn clear_window(&mut self) {
        self.window = None;
    }

    /// Installs `widget` as the content of node `id`.
    pub(super) fn set_node<W: GlWidget + 'static>(&mut self, id: WidgetId, widget: W) {
        self.nodes.insert(id.raw(), Rc::new(widget));
    }

    /// Removes the content of node `id`.
    pub(super) fn clear_node(&mut self, id: WidgetId) {
        self.nodes.remove(&id.raw());
    }

    /// A snapshot of the content, taken before painting so a widget's fallback
    /// paint can re-enter the backend without the window map staying borrowed.
    pub(super) fn snapshot(&self) -> GlSnapshot {
        GlSnapshot {
            width: self.width,
            height: self.height,
            window: self.window.clone(),
            nodes: self
                .nodes
                .iter()
                .map(|(raw, widget)| (*raw, Rc::clone(widget)))
                .collect(),
        }
    }
}

/// A cloned view of a window's GL content, safe to paint after the window map
/// borrow is released.
#[cfg(feature = "winit-backend")]
pub(super) struct GlSnapshot {
    width: i32,
    height: i32,
    window: Option<Rc<dyn GlWidget>>,
    nodes: Vec<(u64, Rc<dyn GlWidget>)>,
}

#[cfg(feature = "winit-backend")]
impl GlSnapshot {
    /// Paints the window-level widget's software fallback as the base layer.
    pub(super) fn paint_window(&self, surface: &mut Surface, dpi: u32, theme: &Theme) {
        if let Some(widget) = &self.window {
            let bounds = Rect::new(0, 0, self.width, self.height);
            surface.with_canvas_at(bounds, dpi, |canvas| widget.paint(canvas, bounds, theme));
        }
    }

    /// Whether node `id` has GL content.
    pub(super) fn has_node(&self, id: WidgetId) -> bool {
        self.nodes.iter().any(|(raw, _)| *raw == id.raw())
    }

    /// Paints node `id`'s software fallback at `bounds`, clipped to `clip`.
    pub(super) fn paint_node(
        &self,
        id: WidgetId,
        surface: &mut Surface,
        bounds: Rect,
        clip: Option<Rect>,
        dpi: u32,
        theme: &Theme,
    ) {
        let Some((_, widget)) = self.nodes.iter().find(|(raw, _)| *raw == id.raw()) else {
            return;
        };
        surface.with_canvas_at(bounds, dpi, |canvas| {
            if let Some(clip) = clip {
                canvas.push_clip(clip);
            }
            widget.paint(canvas, bounds, theme);
        });
    }
}

/// With the windowed backend off there is no `GlWidget` seam, so an offscreen
/// window carries no GL content.
#[cfg(not(feature = "winit-backend"))]
pub(super) struct GlContent;

#[cfg(not(feature = "winit-backend"))]
impl GlContent {
    /// Empty GL content; `width`/`height` are unused without the seam.
    pub(super) fn new(_width: i32, _height: i32) -> GlContent {
        GlContent
    }

    /// A snapshot that never reports content.
    pub(super) fn snapshot(&self) -> GlSnapshot {
        GlSnapshot
    }
}

/// A no-op snapshot when the GL seam is compiled out.
#[cfg(not(feature = "winit-backend"))]
pub(super) struct GlSnapshot;

#[cfg(not(feature = "winit-backend"))]
impl GlSnapshot {
    /// Paints nothing.
    pub(super) fn paint_window(&self, _surface: &mut Surface, _dpi: u32, _theme: &Theme) {}

    /// Never has content.
    pub(super) fn has_node(&self, _id: WidgetId) -> bool {
        false
    }

    /// Paints nothing.
    pub(super) fn paint_node(
        &self,
        _id: WidgetId,
        _surface: &mut Surface,
        _bounds: Rect,
        _clip: Option<Rect>,
        _dpi: u32,
        _theme: &Theme,
    ) {
    }
}
