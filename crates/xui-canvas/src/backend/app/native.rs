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
use crate::backend::native::placement_change;
use crate::geometry;
use crate::sys::child_window;

impl App<'_> {
    /// Makes, moves, shows and hides the native child windows to match their
    /// nodes, and drops those whose node or parent window is gone.
    pub(super) fn reconcile_native(&mut self, event_loop: &ActiveEventLoop) {
        let ids: Vec<WidgetId> = self.shared.native.borrow().keys().copied().collect();
        for id in ids {
            let Some((raw, rect, shown)) = self.native_place(id) else {
                self.shared.set_native(id, false);
                continue;
            };
            let Some(parent) = self.window_handle(raw) else {
                self.shared.set_native(id, false);
                continue;
            };
            let mut children = self.shared.native.borrow_mut();
            let Some(child) = children.get_mut(&id) else {
                continue;
            };
            if child.window.is_none() {
                // A failure is not retried every turn of the loop: the node
                // keeps drawing through its painter, as on a backend without.
                match child_window::create(event_loop, &parent) {
                    Ok(window) => {
                        let locked = *self.shared.pointer_lock.borrow();
                        let ours = locked.is_some_and(|lock| self.same_window(lock, id));
                        window.window().set_cursor_visible(!ours);
                        child.window = Some(Arc::new(window));
                    }
                    Err(_) => {
                        drop(children);
                        self.shared.set_native(id, false);
                        continue;
                    }
                }
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

    /// `id`'s parent window, its rectangle there (cut to its ancestors'
    /// clips) and whether it is shown; `None` when the node is gone.
    fn native_place(&self, id: WidgetId) -> Option<(u64, Rect, bool)> {
        let nodes = self.shared.nodes.borrow();
        let raw = nodes.window_of(id)?.raw();
        let placement = geometry::placement(&nodes, id)?;
        let rect = placement.visible_bounds();
        Some((raw, rect, placement.visible && !rect.is_empty()))
    }

    fn same_window(&self, a: WidgetId, b: WidgetId) -> bool {
        let nodes = self.shared.nodes.borrow();
        nodes.window_of(a).is_some() && nodes.window_of(a) == nodes.window_of(b)
    }

    /// Passes an event of a native child window on to its parent window as
    /// pointer input there; false when `window_id` is not a child's.
    pub(super) fn native_event(
        &mut self,
        window_id: winit::window::WindowId,
        event: &WindowEvent,
    ) -> bool {
        let Some((raw, offset)) = self.native_parent(window_id) else {
            return false;
        };
        match *event {
            WindowEvent::CursorMoved { position, .. } => {
                let (x, y) = (f64::from(offset.0), f64::from(offset.1));
                self.cursor_moved(raw, position.x + x, position.y + y);
            }
            WindowEvent::MouseInput { state, button, .. } => self.mouse_input(raw, state, button),
            WindowEvent::MouseWheel { delta, .. } => self.mouse_wheel(raw, delta),
            _ => {}
        }
        true
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
