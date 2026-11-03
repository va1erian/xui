#![forbid(unsafe_code)]

//! Painting a window: the theme background, then every visible node's painter
//! in creation order, each clipped to its reach and to what Windows says needs
//! repainting.

use std::rc::Rc;

use xui_core::backend::{Painter, WidgetId, WindowId};
use xui_core::geometry::Rect;

use super::Shared;
use super::canvas::D2dCanvas;
use super::scene::intersect;
use crate::sys::gfx::{Frame, RectF, Target};
use crate::sys::window;

/// How far a painter's pixels may reach outside its node, in design units:
/// strokes are centred on an edge and focus rings and glows go further. The
/// software backend uses the same margin, so a widget draws the same on both.
const OVERDRAW_DIP: i32 = 8;

/// One node to paint in a frame.
struct Draw {
    painter: Painter,
    bounds: Rect,
    reach: Rect,
}

impl Shared {
    /// Repaints a node's whole area, and the margin its painter may overdraw.
    pub(super) fn invalidate(&self, id: WidgetId) {
        let area = self.scene.borrow().absolute_bounds(id);
        if let (Some(window), Some(area)) = (self.window_of(id), area) {
            self.invalidate_area(window, area);
        }
    }

    /// Repaints `area` (window coordinates) and the overdraw margin around it.
    pub(super) fn invalidate_area(&self, window: WindowId, area: Rect) {
        let margin = OVERDRAW_DIP * self.dpi_of(window) as i32 / 96;
        if let Some(hwnd) = self.hwnd_of(window) {
            let grown = Rect::new(
                area.left - margin,
                area.top - margin,
                area.right + margin,
                area.bottom + margin,
            );
            window::invalidate(hwnd, Some(grown));
        }
    }

    pub(super) fn invalidate_window(&self, window: WindowId) {
        if let Some(hwnd) = self.hwnd_of(window) {
            window::invalidate(hwnd, None);
        }
    }

    /// The target of `window`, created on first use.
    fn target_of(&self, window: WindowId) -> Option<Rc<Target>> {
        let mut windows = self.windows.borrow_mut();
        let state = windows.get_mut(&window.raw())?;
        if state.target.is_none() {
            state.target = Target::new(state.hwnd, state.size.0, state.size.1).map(Rc::new);
        }
        state.target.clone()
    }

    /// The nodes of `window` that paint inside `dirty`, in paint order.
    fn plan(&self, window: WindowId, dirty: Rect, dpi: u32) -> Vec<Draw> {
        let margin = OVERDRAW_DIP * dpi as i32 / 96;
        let scene = self.scene.borrow();
        scene
            .iter()
            .filter(|(_, node)| node.window == window)
            .filter_map(|(id, node)| {
                let painter = node.painter.clone()?;
                if !scene.effectively_visible(id) {
                    return None;
                }
                let bounds = scene.absolute_bounds(id)?;
                let grown = if node.contained {
                    bounds
                } else {
                    Rect::new(
                        bounds.left - margin,
                        bounds.top - margin,
                        bounds.right + margin,
                        bounds.bottom + margin,
                    )
                };
                let mut reach = intersect(grown, dirty);
                if let Some(clip) = scene.ancestor_clip(id) {
                    reach = intersect(reach, clip);
                }
                (!reach.is_empty()).then_some(Draw {
                    painter,
                    bounds,
                    reach,
                })
            })
            .collect()
    }

    /// Handles `WM_PAINT` for `window`.
    pub(super) fn paint(&self, window: WindowId) {
        let Some(hwnd) = self.hwnd_of(window) else {
            return;
        };
        window::paint(hwnd, |dirty| self.paint_dirty(window, dirty));
    }

    fn paint_dirty(&self, window: WindowId, dirty: Rect) {
        let Some(target) = self.target_of(window) else {
            return;
        };
        let (dpi, size, theme) = {
            let windows = self.windows.borrow();
            let Some(state) = windows.get(&window.raw()) else {
                return;
            };
            (state.dpi, state.size, state.theme)
        };
        let bounds = Rect::new(0, 0, size.0, size.1);
        let dirty = intersect(dirty, bounds);
        if dirty.is_empty() {
            return;
        }
        let draws = self.plan(window, dirty, dpi);

        target.begin();
        target.push_clip(RectF::from(dirty));
        {
            let mut canvas = D2dCanvas::new(Rc::clone(&target), bounds, dpi, false);
            xui_core::theme::look::paint_background(&mut canvas, dirty, bounds, &theme);
        }
        for draw in &draws {
            target.set_origin(0.0, 0.0);
            target.push_clip(RectF::from(draw.reach));
            target.set_origin(draw.bounds.left as f32, draw.bounds.top as f32);
            let local = Rect::new(0, 0, draw.bounds.width(), draw.bounds.height());
            let mut canvas = D2dCanvas::new(Rc::clone(&target), local, dpi, true);
            (draw.painter)(&mut canvas);
            drop(canvas);
            target.set_origin(0.0, 0.0);
            target.pop_clip();
        }
        target.set_origin(0.0, 0.0);
        target.pop_clip();
        if target.end() == Frame::Lost {
            if let Some(state) = self.windows.borrow_mut().get_mut(&window.raw()) {
                state.target = None;
            }
            self.invalidate_window(window);
        }
    }
}
