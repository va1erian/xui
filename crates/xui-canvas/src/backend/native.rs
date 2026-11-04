#![forbid(unsafe_code)]

//! Native child windows that nodes own, for a GPU API to present into: which
//! nodes asked for one, the OS window once the event loop has made it, and
//! where it was last put.
//!
//! The handler makes and places them ([`super::app`]); the backend methods only
//! record what the app asked for, since a `winit` window can be made only
//! while the event loop runs.

use std::sync::Arc;

use xui_core::backend::{NativeSurface, WidgetId};
use xui_core::geometry::Rect;

use super::Shared;
use crate::sys::child_window::ChildWindow;

/// One node's native child window.
#[derive(Default)]
pub(crate) struct NativeChild {
    /// The OS window, once made.
    pub(crate) window: Option<Arc<ChildWindow>>,
    /// Where it was last put, in its parent's client pixels, and whether it
    /// was shown; `None` before it was first placed.
    pub(crate) placed: Option<(Rect, bool)>,
}

impl Shared {
    /// Records that `id` wants a native child window (`on`), or gives its
    /// window up: hidden at once, as the holder may keep it alive for a while.
    pub(crate) fn set_native(&self, id: WidgetId, on: bool) {
        let mut children = self.native.borrow_mut();
        if on {
            children.entry(id).or_default();
            return;
        }
        if let Some(window) = children.remove(&id).and_then(|child| child.window) {
            window.window().set_visible(false);
        }
    }

    /// The native child window `id` was given, once it exists.
    pub(crate) fn native_surface(&self, id: WidgetId) -> Option<NativeSurface> {
        let window = self.native.borrow().get(&id)?.window.clone()?;
        Some(window)
    }

    /// Shows (`true`) or hides the cursor over every native child window of
    /// `id`'s OS window, which owns the cursor where it is over them.
    pub(crate) fn native_cursor_visible(&self, id: WidgetId, visible: bool) {
        let Some(window) = self.nodes.borrow().window_of(id) else {
            return;
        };
        let nodes = self.nodes.borrow();
        for (child, native) in self.native.borrow().iter() {
            if nodes.window_of(*child) == Some(window)
                && let Some(handle) = &native.window
            {
                handle.window().set_cursor_visible(visible);
            }
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_first_placement_moves_and_shows() {
        let rect = Rect::new(10, 20, 110, 220);
        assert_eq!(placement_change(None, rect, true), (Some(rect), Some(true)));
    }

    #[test]
    fn an_unchanged_placement_changes_nothing() {
        let rect = Rect::new(10, 20, 110, 220);
        assert_eq!(
            placement_change(Some((rect, true)), rect, true),
            (None, None)
        );
    }

    #[test]
    fn a_hidden_node_hides_and_keeps_its_place() {
        let rect = Rect::new(10, 20, 110, 220);
        let gone = Rect::default();
        assert_eq!(
            placement_change(Some((rect, true)), gone, false),
            (None, Some(false))
        );
    }

    #[test]
    fn a_moved_node_moves_without_showing_again() {
        let (a, b) = (Rect::new(0, 0, 50, 50), Rect::new(5, 0, 60, 50));
        assert_eq!(placement_change(Some((a, true)), b, true), (Some(b), None));
    }
}
