#![forbid(unsafe_code)]

//! Native child windows that nodes own, for a GPU API to present into: which
//! nodes asked for one, the OS window once the event loop has made it, and
//! where it was last put.
//!
//! The handler makes and places them ([`super::app`]); the backend methods only
//! record what the app asked for, since a `winit` window can be made only
//! while the event loop runs.

use std::sync::Arc;

use winit::window::CursorIcon;
use xui_core::backend::{NativeSurface, WidgetId};
use xui_core::geometry::Rect;

use super::Shared;
use crate::geometry::{Placement, intersect};
use crate::sys::child_window::ChildWindow;

/// One node's native child window.
#[derive(Default)]
pub(crate) struct NativeChild {
    /// The OS window, once made.
    pub(crate) window: Option<Arc<ChildWindow>>,
    /// Where it was last put, in its parent's client pixels, and whether it
    /// was shown; `None` before it was first placed.
    pub(crate) placed: Option<(Rect, bool)>,
    /// The window could not be made; it is not tried again for this request.
    pub(crate) failed: bool,
}

impl Shared {
    /// Records that `id` wants a native child window (`on`), or gives its
    /// window up: hidden at once, and its handles refused from then on, as the
    /// holder may keep it alive for a while and its parent window may close
    /// meanwhile, which the backend would no longer hear about.
    pub(crate) fn set_native(&self, id: WidgetId, on: bool) {
        let mut children = self.native.borrow_mut();
        if on {
            children.entry(id).or_default();
            return;
        }
        if let Some(window) = children.remove(&id).and_then(|child| child.window) {
            window.window().set_visible(false);
            window.orphan();
        }
    }

    /// The native child window `id` was given, once it exists.
    pub(crate) fn native_surface(&self, id: WidgetId) -> Option<NativeSurface> {
        let window = self.native.borrow().get(&id)?.window.clone()?;
        Some(window)
    }

    /// The native child windows of the OS window `raw`.
    pub(crate) fn native_windows_of(&self, raw: u64) -> Vec<Arc<ChildWindow>> {
        let nodes = self.nodes.borrow();
        self.native
            .borrow()
            .iter()
            .filter(|(id, _)| nodes.window_of(**id).is_some_and(|w| w.raw() == raw))
            .filter_map(|(_, child)| child.window.clone())
            .collect()
    }

    /// Shows (`true`) or hides the cursor over every native child window of
    /// `id`'s OS window, which owns the cursor where it is over them.
    pub(crate) fn native_cursor_visible(&self, id: WidgetId, visible: bool) {
        let Some(window) = self.nodes.borrow().window_of(id) else {
            return;
        };
        for child in self.native_windows_of(window.raw()) {
            child.window().set_cursor_visible(visible);
        }
    }

    /// Shows `icon` over the native child windows of `raw` too, as the parent's
    /// cursor does not reach over them.
    pub(crate) fn native_cursor(&self, raw: u64, icon: CursorIcon) {
        if self.native.borrow().is_empty() {
            return;
        }
        for child in self.native_windows_of(raw) {
            child.window().set_cursor(icon);
        }
    }
}

/// Where a node's native window goes (its bounds) and whether it is shown: only
/// while the node is visible, has an area, lies wholly inside its clips (a
/// window cannot be clipped the way a painter is, and a part shown would not be
/// the part the app drew) and no popup lies over it (a popup is drawn in the
/// parent, under the window).
pub(crate) fn native_place(placement: &Placement, popups: &[Rect]) -> (Rect, bool) {
    let bounds = placement.bounds;
    let whole = placement
        .clip
        .is_none_or(|clip| intersect(bounds, clip) == bounds);
    let covered = popups.iter().any(|p| !intersect(*p, bounds).is_empty());
    let shown = placement.visible && !bounds.is_empty() && whole && !covered;
    (bounds, shown)
}

/// What to change to put a native window at `rect` (shown or not), from where
/// it was last put: its position and size, and its visibility; `None` for what
/// stays as it was.
pub(crate) fn placement_change(
    placed: Option<(Rect, bool)>,
    rect: Rect,
    shown: bool,
) -> (Option<Rect>, Option<bool>) {
    let moved = (placed.map(|(r, _)| r) != Some(rect) && !rect.is_empty()).then_some(rect);
    let shown = (placed.map(|(_, s)| s) != Some(shown)).then_some(shown);
    (moved, shown)
}

/// A point in a child window at `origin` of its parent, in the parent's pixels.
pub(crate) fn to_parent((x, y): (f64, f64), origin: (i32, i32)) -> (f64, f64) {
    (x + f64::from(origin.0), y + f64::from(origin.1))
}

#[cfg(test)]
#[path = "native_tests.rs"]
mod tests;
