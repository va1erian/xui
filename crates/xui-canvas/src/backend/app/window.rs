#![forbid(unsafe_code)]

//! Real-window creation and per-window DPI: turning the front layer's requested
//! windows into `winit` windows, and keeping their size and node bounds in step
//! with the monitor's scale factor.

use std::rc::Rc;

use winit::dpi::LogicalSize;
use winit::event_loop::ActiveEventLoop;
use winit::window::{Window, WindowAttributes};

use xui_core::backend::Decorations;
use xui_core::geometry::Rect;

use super::App;
use crate::backend::software::RealWindow;
use crate::backend::{DEFAULT_DPI, Shared};

impl App {
    /// Creates the OS window for any backend window that lacks one. Its
    /// presentation surface is created lazily on the first frame.
    pub(super) fn create_windows(&mut self, event_loop: &ActiveEventLoop) {
        let ids: Vec<u64> = self.shared.windows.borrow().keys().copied().collect();
        for raw in ids {
            if self.windows.contains_key(&raw) {
                continue;
            }
            let (title, size, decorations) = {
                let windows = self.shared.windows.borrow();
                let Some(state) = windows.get(&raw) else {
                    continue;
                };
                (state.title.clone(), state.size, state.decorations)
            };
            // A requested backdrop (Acrylic/Mica) is approximated by the opaque
            // theme background on platforms without the DWM material; softbuffer
            // presents an opaque surface, so there is nothing else to do.
            let attributes = WindowAttributes::default()
                .with_title(title)
                .with_inner_size(LogicalSize::new(f64::from(size.0), f64::from(size.1)))
                .with_decorations(decorations == Decorations::System);
            let Ok(window) = event_loop.create_window(attributes) else {
                continue;
            };
            let window = Rc::new(window);
            let metrics = window_metrics(&window);
            if let Some(state) = self.shared.windows.borrow_mut().get_mut(&raw) {
                state.size = metrics.0;
                state.window = Some(Rc::clone(&window));
            }
            self.set_dpi(raw, metrics.1);
            self.windows.insert(raw, RealWindow::new());
            self.redraw(raw);
        }
    }

    /// Records `raw`'s dots-per-inch, lifting the node bounds the app laid out
    /// at the old value to the new scale so the window stays filled.
    ///
    /// `run_app` builds the widgets before the real window exists, so they are
    /// laid out at [`DEFAULT_DPI`]; the first real window reports the
    /// monitor's scale factor (2.0 on a Retina display) and the bounds move to
    /// the backing pixels. Later scale changes (a window dragged to another
    /// monitor) rescale by the ratio, since the portable widgets do not reflow
    /// themselves.
    pub(super) fn set_dpi(&self, raw: u64, dpi: u32) {
        let old = {
            let mut windows = self.shared.windows.borrow_mut();
            let Some(state) = windows.get_mut(&raw) else {
                return;
            };
            let old = state.dpi;
            state.dpi = dpi;
            old
        };
        if old != dpi {
            rescale_nodes(&self.shared, raw, dpi as f32 / old as f32);
        }
    }
}

/// The dots-per-inch a `winit` scale factor corresponds to (96 at 100%).
pub(super) fn dpi_from_scale(scale_factor: f64) -> u32 {
    (scale_factor * f64::from(DEFAULT_DPI)).round().max(1.0) as u32
}

/// The window's `(size, dpi)` from its scale factor.
fn window_metrics(window: &Window) -> ((u32, u32), u32) {
    let size = window.inner_size();
    let dpi = dpi_from_scale(window.scale_factor());
    ((size.width.max(1), size.height.max(1)), dpi)
}

/// Multiplies `rect`'s edges by `ratio`, rounding to the nearest pixel.
fn scale_rect(rect: Rect, ratio: f32) -> Rect {
    let scale = |value: i32| (value as f32 * ratio).round() as i32;
    Rect::new(
        scale(rect.left),
        scale(rect.top),
        scale(rect.right),
        scale(rect.bottom),
    )
}

/// Rescales every node of `raw` from one layout scale to another.
fn rescale_nodes(shared: &Shared, raw: u64, ratio: f32) {
    if (ratio - 1.0).abs() < f32::EPSILON {
        return;
    }
    for (_, node) in shared.nodes.borrow_mut().iter_mut() {
        if node.window.raw() == raw {
            node.bounds = scale_rect(node.bounds, ratio);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scale_factor_maps_to_a_dots_per_inch() {
        assert_eq!(dpi_from_scale(1.0), 96);
        assert_eq!(dpi_from_scale(1.25), 120);
        assert_eq!(dpi_from_scale(2.0), 192, "a Retina display is 2x");
        assert_eq!(dpi_from_scale(0.0), 1, "never zero");
    }

    #[test]
    fn scaling_a_rect_lifts_the_backing_store() {
        let rect = Rect::new(16, 12, 380, 40);
        assert_eq!(scale_rect(rect, 2.0), Rect::new(32, 24, 760, 80));
        assert_eq!(scale_rect(rect, 1.25), Rect::new(20, 15, 475, 50));
    }
}
