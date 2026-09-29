#![forbid(unsafe_code)]

//! The Win32 implementation of the [`Backend`](xui_core::backend::Backend)
//! contract.
//!
//! Most nodes are painted child windows: the front layer registers a painter
//! and the node's handler runs it on `WM_PAINT`. Kinds with a good native
//! control are hosted natively instead — today
//! [`NodeKind::Edit`](xui_core::backend::NodeKind::Edit) is a real `EDIT`,
//! whose changes reach the widget as
//! [`Event::TextChanged`](xui_core::backend::Event::TextChanged) — and
//! [`Win32Backend::supports`] reports
//! [`ImplKind::Native`](xui_core::backend::ImplKind::Native) for them.

pub(crate) mod canvas;
mod chrome;
mod contract;
mod cursor;
mod handler;
mod handler_node;
mod node;
mod node_ops;
mod shape;
mod text;
mod theme;
mod window_ops;

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use xui_core::Theme;
use xui_core::backend::{BackendError, ParentRef, Result as BackendResult, WidgetId, WindowId};

use crate::window::Window;

use handler::WindowShared;
use node::BackendNode;

/// A top-level window the backend created.
struct BackendWindow {
    window: Window,
    shared: Rc<WindowShared>,
    theme: Cell<Theme>,
}

/// The Win32 backend.
pub struct Win32Backend {
    windows: RefCell<HashMap<u64, BackendWindow>>,
    nodes: RefCell<HashMap<u64, BackendNode>>,
    next_window: Cell<u64>,
    next_widget: Cell<u64>,
}

impl Default for Win32Backend {
    fn default() -> Win32Backend {
        Win32Backend::new()
    }
}

impl Win32Backend {
    /// A backend with no windows yet. Ids start at one so the first handle is
    /// never the null id.
    pub fn new() -> Win32Backend {
        Win32Backend {
            windows: RefCell::new(HashMap::new()),
            nodes: RefCell::new(HashMap::new()),
            next_window: Cell::new(1),
            next_widget: Cell::new(1),
        }
    }

    fn allocate(cell: &Cell<u64>) -> u64 {
        let id = cell.get();
        cell.set(id + 1);
        id
    }

    /// Resolves a parent to the handle to parent under, its owning window, and
    /// that window's shared state.
    fn resolve_parent(
        &self,
        parent: ParentRef,
    ) -> BackendResult<(crate::hwnd::Hwnd, WindowId, Rc<WindowShared>)> {
        match parent {
            ParentRef::Window(window) => {
                let windows = self.windows.borrow();
                let entry = windows
                    .get(&window.raw())
                    .ok_or(BackendError::CreateFailed("window"))?;
                Ok((entry.window.hwnd(), window, Rc::clone(&entry.shared)))
            }
            ParentRef::Widget(widget) => {
                let (hwnd, window) = {
                    let nodes = self.nodes.borrow();
                    let node = nodes
                        .get(&widget.raw())
                        .ok_or(BackendError::CreateFailed("parent node"))?;
                    (node.hwnd, node.window_id)
                };
                let shared = {
                    let windows = self.windows.borrow();
                    let entry = windows
                        .get(&window.raw())
                        .ok_or(BackendError::CreateFailed("window"))?;
                    Rc::clone(&entry.shared)
                };
                Ok((hwnd, window, shared))
            }
        }
    }

    fn node(&self, id: WidgetId) -> Option<(crate::hwnd::Hwnd, WindowId)> {
        self.nodes
            .borrow()
            .get(&id.raw())
            .map(|node| (node.hwnd, node.window_id))
    }

    /// Forgets a node's child handle in its window's index, so a later
    /// `WM_COMMAND` cannot reach a destroyed widget.
    fn unregister(&self, id: &WidgetId) {
        let entry = {
            let nodes = self.nodes.borrow();
            nodes.get(&id.raw()).map(|node| (node.hwnd, node.window_id))
        };
        if let Some((hwnd, window_id)) = entry
            && let Some(window) = self.windows.borrow().get(&window_id.raw())
        {
            window.shared.unregister_node(hwnd);
            window.shared.forget_popup(hwnd);
            window.shared.clear_key_focus(*id);
        }
    }

    /// The handle behind a top-level window, for interop with the platform
    /// layer (capture, message hooks). `None` once it is closed.
    pub fn window_hwnd(&self, window: WindowId) -> Option<crate::hwnd::Hwnd> {
        self.windows
            .borrow()
            .get(&window.raw())
            .map(|entry| entry.window.hwnd())
    }

    /// Sets `window`'s large and small icons from `icon`. Does nothing once the
    /// window is closed.
    ///
    /// Windows keeps a reference to the icon rather than copying it, so `icon`
    /// must outlive the window.
    pub fn set_window_icon(&self, window: WindowId, icon: &crate::Icon) {
        if let Some(entry) = self.windows.borrow().get(&window.raw()) {
            entry.window.set_icon(icon);
        }
    }

    /// The handle behind a node, for interop with the platform layer.
    pub fn node_hwnd(&self, id: WidgetId) -> Option<crate::hwnd::Hwnd> {
        self.node(id).map(|(hwnd, _)| hwnd)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use xui_core::backend::{Backend, PlatformSpec};

    #[test]
    fn set_window_icon_installs_both_sizes() {
        let backend = Win32Backend::new();
        backend.init();
        let window = backend
            .open_window(&PlatformSpec::new("xui.icon"))
            .expect("the icon test requires a desktop");
        let icon = crate::Icon::from_rgba(16, 16, &[0xFF; 16 * 16 * 4]).expect("icon");
        backend.set_window_icon(window, &icon);
        let hwnd = backend.window_hwnd(window).expect("hwnd");
        assert!(crate::sys::window_icon::has_icon(hwnd, true));
        assert!(crate::sys::window_icon::has_icon(hwnd, false));
        backend.close_window(window);
    }
}
