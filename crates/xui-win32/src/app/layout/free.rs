#![forbid(unsafe_code)]

//! The absolute placement pass behind [`Layout::free`](super::Layout::free):
//! every widget keeps its (design) bounds and follows the parent's resize per
//! its [`Anchor`](crate::layout::Anchor), so an app can pin edges and centres
//! without reimplementing the arithmetic.
//!
//! The design bounds are captured in device pixels at 96 DPI and re-scaled to
//! the current DPI on every pass, so a `WM_DPICHANGED` re-scales the layout
//! instead of re-reading the already-moved controls.

use crate::geometry::{Rect, Size};
use crate::layout::anchored;
use crate::units::Dip;

use super::item::Content;
use super::{Layout, Placed};

/// Places every widget of a free `layout` inside `rect` (the parent's current
/// client area, in device pixels), anchored against `origin`.
pub(super) fn compute(layout: &Layout, origin: Size, rect: Rect, dpi: u32) -> Vec<Placed> {
    let origin = scale_size(origin, dpi);
    let resized = rect.size();
    let mut placed = Vec::new();
    for item in &layout.slots {
        if !item.is_visible() {
            continue;
        }
        match item.content() {
            Content::Widget(handle) => {
                let bounds = scale_rect(handle.design_bounds(dpi), dpi);
                placed.push(Placed {
                    handle: handle.clone(),
                    rect: anchored(origin, resized, bounds, item.anchor()),
                });
            }
            // A free layout positions widgets; a nested node or subtree fills
            // the parent and runs its own pass.
            _ => item.compute(rect, dpi, &mut placed),
        }
    }
    placed
}

/// The preferred size of a free `layout`, in device pixels at `dpi`: the
/// furthest right and bottom edge among its widgets' design bounds (in device
/// pixels at 96 DPI) and the design `origin` itself.
///
/// The origin is included so a free layout never packs smaller than the window
/// it was designed against, only grows to fit content pinned outside it.
pub(super) fn preferred(layout: &Layout, origin: Size, dpi: u32) -> Size {
    let mut bounds = vec![Rect::from_size(origin)];
    for item in &layout.slots {
        if !item.is_visible() {
            continue;
        }
        if let Content::Widget(handle) = item.content() {
            bounds.push(handle.design_bounds(dpi));
        }
    }
    let extent = crate::layout::free_preferred(&bounds);
    Size::new(scale(extent.width, dpi), scale(extent.height, dpi))
}

/// Scales a design value (device pixels at 96 DPI) to device pixels at `dpi`.
fn scale(value: i32, dpi: u32) -> i32 {
    Dip::new(value as f32).to_px(dpi).value()
}

fn scale_size(size: Size, dpi: u32) -> Size {
    Size::new(scale(size.width, dpi), scale(size.height, dpi))
}

fn scale_rect(rect: Rect, dpi: u32) -> Rect {
    Rect::new(
        scale(rect.left, dpi),
        scale(rect.top, dpi),
        scale(rect.right, dpi),
        scale(rect.bottom, dpi),
    )
}
