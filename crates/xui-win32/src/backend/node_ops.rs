#![forbid(unsafe_code)]

//! Node-level [`Backend`](xui_core::backend::Backend) bodies: create, destroy,
//! move, show and focus a node. Split from `contract.rs` (which delegates to
//! these) so both files stay under the size limit.

use std::rc::Rc;

use xui_core::backend::{NodeSpec, ParentRef, Result as BackendResult, WidgetId, WindowId};
use xui_core::{Point, Rect};

use super::Win32Backend;
use super::handler::WindowShared;
use super::node::BackendNode;
use crate::sys;

impl Win32Backend {
    /// Creates a node under `parent`; the body of
    /// [`Backend::create`](xui_core::backend::Backend::create).
    pub(super) fn create_node(
        &self,
        parent: ParentRef,
        spec: &NodeSpec,
    ) -> BackendResult<WidgetId> {
        let (parent_hwnd, window_id, shared) = self.resolve_parent(parent)?;
        let widget = WidgetId::from_raw(Self::allocate(&self.next_widget));
        let node = BackendNode::create(window_id, parent_hwnd, parent, &shared, widget, spec)?;
        self.nodes.borrow_mut().insert(widget.raw(), node);
        Ok(widget)
    }

    /// Destroys a node and any descendants orphaned by it; the body of
    /// [`Backend::destroy`](xui_core::backend::Backend::destroy).
    pub(super) fn destroy_node(&self, id: WidgetId) {
        self.unregister(&id);
        self.nodes.borrow_mut().remove(&id.raw());
        // Cascade: a container's children may be destroyed with it, so drop
        // every node whose parent chain no longer exists.
        loop {
            let doomed: Vec<u64> = {
                let nodes = self.nodes.borrow();
                nodes
                    .iter()
                    .filter(|(_, node)| match node.parent {
                        ParentRef::Window(window) => {
                            !self.windows.borrow().contains_key(&window.raw())
                        }
                        ParentRef::Widget(parent) => !nodes.contains_key(&parent.raw()),
                    })
                    .map(|(id, _)| *id)
                    .collect()
            };
            if doomed.is_empty() {
                break;
            }
            for id in &doomed {
                self.unregister(&WidgetId::from_raw(*id));
            }
            let mut nodes = self.nodes.borrow_mut();
            for id in doomed {
                nodes.remove(&id);
            }
        }
    }

    /// Applies a layout move list; the body of
    /// [`Backend::apply_moves`](xui_core::backend::Backend::apply_moves).
    pub(super) fn apply_node_moves(&self, _window: WindowId, moves: &[(WidgetId, Rect)]) {
        let os_moves: Vec<(crate::hwnd::Hwnd, Rect)> = {
            let nodes = self.nodes.borrow();
            let windows = self.windows.borrow();
            for (id, rect) in moves {
                if let Some(node) = nodes.get(&id.raw()) {
                    node.set_bounds(*rect);
                }
            }
            moves
                .iter()
                .filter_map(|(id, rect)| {
                    let node = nodes.get(&id.raw())?;
                    // A popup is a top-level window: the core works in host
                    // client coordinates, so convert its rect to the screen.
                    let rect = if node.is_popup {
                        let entry = windows.get(&node.window_id.raw())?;
                        // Track the host-client rect so the popup follows the
                        // host window on a move.
                        entry.shared.track_popup(node.hwnd, *rect);
                        let at = sys::window::client_to_screen(
                            entry.window.hwnd(),
                            Point::new(rect.left, rect.top),
                        );
                        Rect::new(at.x, at.y, at.x + rect.width(), at.y + rect.height())
                    } else {
                        *rect
                    };
                    Some((node.hwnd, rect))
                })
                .collect()
        };
        sys::layout::apply(&os_moves);
    }

    /// Shows or hides a node; the body of
    /// [`Backend::set_visible`](xui_core::backend::Backend::set_visible).
    pub(super) fn set_node_visible(&self, id: WidgetId, visible: bool) {
        let entry = self
            .nodes
            .borrow()
            .get(&id.raw())
            .map(|node| (node.hwnd, node.is_popup));
        if let Some((hwnd, is_popup)) = entry {
            if visible && is_popup {
                // Paint the finished face before the popup is composed, so its
                // first frame is never the class background. Show it without
                // activating: a plain `SW_SHOW` would steal the host's
                // activation and flicker its chrome on every open/switch.
                sys::first_show::show_painted(hwnd, || {
                    sys::window::show(hwnd, sys::window::ShowKind::NoActivate);
                });
            } else {
                let kind = if visible {
                    sys::window::ShowKind::Normal
                } else {
                    sys::window::ShowKind::Hidden
                };
                sys::window::show(hwnd, kind);
            }
        }
        if !visible {
            self.clear_key_focus(id);
            if let Some((hwnd, true)) = entry
                && let Some(shared) = self.node_shared(id)
            {
                shared.forget_popup(hwnd);
            }
        }
    }

    /// Gives a node the keyboard focus; the body of
    /// [`Backend::focus`](xui_core::backend::Backend::focus).
    pub(super) fn focus_node(&self, id: WidgetId) {
        let Some((hwnd, is_popup, window)) = self
            .nodes
            .borrow()
            .get(&id.raw())
            .map(|node| (node.hwnd, node.is_popup, node.window_id))
        else {
            return;
        };
        let shared = self
            .windows
            .borrow()
            .get(&window.raw())
            .map(|entry| Rc::clone(&entry.shared));
        let Some(shared) = shared else {
            return;
        };
        if is_popup {
            // A top-level popup must not take activation, or the host window
            // would lose focus. Keep the OS focus on the host and route its
            // keyboard input to the popup instead, as a native menu does.
            shared.set_key_focus(Some(id));
            if let Some(host) = self.window_hwnd(window) {
                sys::window::set_focus(host);
            }
        } else {
            shared.set_key_focus(None);
            sys::window::set_focus(hwnd);
        }
    }

    /// The shared state of the window that owns `id`, if it still exists.
    fn node_shared(&self, id: WidgetId) -> Option<Rc<WindowShared>> {
        let window = self
            .nodes
            .borrow()
            .get(&id.raw())
            .map(|node| node.window_id)?;
        self.windows
            .borrow()
            .get(&window.raw())
            .map(|entry| Rc::clone(&entry.shared))
    }

    /// Drops the host window's logical keyboard focus when it is `id`, so a
    /// hidden or destroyed popup stops receiving the host's key input.
    fn clear_key_focus(&self, id: WidgetId) {
        if let Some(shared) = self.node_shared(id) {
            shared.clear_key_focus(id);
        }
    }
}
