#![forbid(unsafe_code)]

//! The handler's side of native child windows: making the ones nodes asked
//! for, keeping each on its node's bounds and visibility, and passing their
//! pointer input on as the node's.

use std::sync::Arc;

use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use xui_core::backend::WidgetId;
use xui_core::geometry::Rect;

use super::App;
use crate::backend::native::{native_place, placement_change, to_parent};
use crate::geometry;
use crate::sys::child_window;

impl App<'_> {
    /// Makes, moves, shows and hides the native child windows to match their
    /// nodes. A child whose node is gone (its window closed, too) is given up
    /// before that window is dropped, so its handles are refused first.
    pub(super) fn reconcile_native(&mut self, event_loop: &ActiveEventLoop) {
        let ids: Vec<WidgetId> = self.shared.native.borrow().keys().copied().collect();
        for id in ids {
            let Some((raw, rect, shown)) = self.native_place(id) else {
                self.shared.set_native(id, false);
                continue;
            };
            // A window opened while the loop runs has no OS window yet.
            let Some(parent) = self.window_handle(raw) else {
                continue;
            };
            let mut children = self.shared.native.borrow_mut();
            let Some(child) = children.get_mut(&id) else {
                continue;
            };
            if child.failed {
                continue;
            }
            if child.window.is_none() {
                // The node keeps drawing through its painter, as on a backend
                // without child windows.
                let Ok(window) = child_window::create(event_loop, &parent) else {
                    child.failed = true;
                    continue;
                };
                let locked = *self.shared.pointer_lock.borrow();
                let ours = locked.is_some_and(|lock| self.same_window(lock, id));
                window.window().set_cursor_visible(!ours);
                child.window = Some(Arc::new(window));
            }
            let Some(window) = child.window.clone() else {
                continue;
            };
            let (moved, visible) = placement_change(child.placed, rect, shown);
            let os = window.window();
            if let Some(r) = moved {
                os.set_outer_position(PhysicalPosition::new(r.left, r.top));
                let _ = os.request_inner_size(PhysicalSize::new(
                    r.width().max(1) as u32,
                    r.height().max(1) as u32,
                ));
            }
            if let Some(v) = visible {
                os.set_visible(v);
            }
            let kept = child.placed.map_or(rect, |(r, _)| r);
            child.placed = Some((moved.unwrap_or(kept), shown));
        }
    }

    /// `id`'s parent window, where its window goes there and whether it is
    /// shown; `None` when the node is gone.
    fn native_place(&self, id: WidgetId) -> Option<(u64, Rect, bool)> {
        let nodes = self.shared.nodes.borrow();
        let window = nodes.window_of(id)?;
        let placement = geometry::placement(&nodes, id)?;
        let popups: Vec<Rect> = nodes
            .iter()
            .filter(|(other, node)| node.popup && node.window == window && *other != id)
            .filter_map(|(other, _)| geometry::placement(&nodes, other))
            .filter(|p| p.visible)
            .map(|p| p.visible_bounds())
            .collect();
        let (rect, shown) = native_place(&placement, &popups);
        Some((window.raw(), rect, shown))
    }

    fn same_window(&self, a: WidgetId, b: WidgetId) -> bool {
        let nodes = self.shared.nodes.borrow();
        nodes.window_of(a).is_some() && nodes.window_of(a) == nodes.window_of(b)
    }

    /// Passes an event of a native child window on to its parent window, as
    /// pointer input there. Events of windows that are no child are ignored.
    pub(super) fn native_event(&mut self, window_id: winit::window::WindowId, event: &WindowEvent) {
        let Some((raw, origin)) = self.native_parent(window_id) else {
            return;
        };
        match *event {
            WindowEvent::CursorMoved { position, .. } => {
                let (x, y) = to_parent((position.x, position.y), origin);
                self.cursor_moved(raw, x, y);
            }
            WindowEvent::CursorLeft { .. } => self.apply_cursor(raw, None),
            WindowEvent::MouseInput { state, button, .. } => self.mouse_input(raw, state, button),
            WindowEvent::MouseWheel { delta, .. } => self.mouse_wheel(raw, delta),
            _ => {}
        }
    }

    /// The parent window of the native child `window_id` and where the child
    /// sits in it.
    fn native_parent(&self, window_id: winit::window::WindowId) -> Option<(u64, (i32, i32))> {
        let children = self.shared.native.borrow();
        let (id, child) = children.iter().find(|(_, child)| {
            child
                .window
                .as_ref()
                .is_some_and(|window| window.window().id() == window_id)
        })?;
        let raw = self.shared.nodes.borrow().window_of(*id)?.raw();
        let (rect, _) = child.placed?;
        Some((raw, (rect.left, rect.top)))
    }
}
