#![forbid(unsafe_code)]

//! Tab-order traversal over the scene, shared by both canvas backends.
//!
//! The order is the paint order of the nodes that asked to be tab stops: creation
//! order unless a node was raised, which sends it to the end of the ring. A stop
//! that another node covers at its centre (the scrim of an open dialog) is not
//! reachable, so the focus cannot leave a modal surface.

use xui_core::backend::{Event, ParentRef, WidgetId, WindowId};
use xui_core::message::Key;

use super::Scene;
use crate::geometry::{absolute_bounds, effectively_enabled, effectively_visible, hit_test};

impl Scene {
    /// The node Tab (or Shift+Tab when `backwards`) moves the focus to from
    /// `focused` in `window`, wrapping at either end.
    ///
    /// `None` when nothing can take the focus, and when the focus holder keeps
    /// Tab for itself: a node that wants Tab (a code editor indents with it) or
    /// a popup surface (a menu is left by Escape, not Tab).
    pub(crate) fn tab_destination(
        &self,
        window: WindowId,
        focused: Option<WidgetId>,
        backwards: bool,
    ) -> Option<WidgetId> {
        let holder = focused.and_then(|id| self.get(id));
        if holder.is_some_and(|node| node.wants_tab || node.popup) {
            return None;
        }
        let stops: Vec<WidgetId> = self
            .iter()
            .filter(|(id, node)| {
                node.window == window
                    && node.tab_stop
                    && effectively_visible(self, *id)
                    && effectively_enabled(self, *id)
                    && self.is_uncovered(*id)
            })
            .map(|(id, _)| id)
            .collect();
        let last = stops.len().checked_sub(1)?;
        let position = focused.and_then(|id| stops.iter().position(|stop| *stop == id));
        let index = match (position, backwards) {
            (Some(at), false) => (at + 1) % stops.len(),
            (Some(at), true) => at.checked_sub(1).unwrap_or(last),
            (None, false) => 0,
            (None, true) => last,
        };
        Some(stops[index])
    }

    /// Whether the topmost node at the centre of `id` is `id` or one of its
    /// descendants, i.e. nothing is painted over it.
    fn is_uncovered(&self, id: WidgetId) -> bool {
        let (Some(node), Some(bounds)) = (self.get(id), absolute_bounds(self, id)) else {
            return false;
        };
        let (x, y) = (
            bounds.left + bounds.width() / 2,
            bounds.top + bounds.height() / 2,
        );
        let Some((top, _, _)) = hit_test(self, node.window, x, y) else {
            return false;
        };
        self.is_within(top, id)
    }

    /// Whether `node` is `ancestor` or created inside it.
    fn is_within(&self, node: WidgetId, ancestor: WidgetId) -> bool {
        let mut current = Some(node);
        while let Some(id) = current {
            if id == ancestor {
                return true;
            }
            current = match self.get(id).map(|node| node.parent) {
                Some(ParentRef::Widget(parent)) => Some(parent),
                _ => None,
            };
        }
        false
    }
}

/// Whether `event` is a plain Tab or Shift+Tab press, which moves the focus,
/// and whether it goes backwards. Ctrl+Tab and Alt+Tab are left to the widget.
pub(crate) fn tab_press(event: &Event) -> Option<bool> {
    match event {
        Event::KeyDown {
            key: Key::TAB,
            modifiers,
            ..
        } if !(modifiers.ctrl || modifiers.alt || modifiers.win) => Some(modifiers.shift),
        _ => None,
    }
}
