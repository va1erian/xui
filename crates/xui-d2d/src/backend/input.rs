#![forbid(unsafe_code)]

//! Pointer routing and the keyboard focus: hit testing, hover, capture, and
//! the order of `SetFocus` / `KillFocus` / `MouseLeave` a widget expects.

use xui_core::backend::{Event, WidgetId, WindowId};

use super::Shared;

/// `event` with its position replaced, for delivery in a node's coordinates.
fn translate(event: Event, x: i32, y: i32) -> Event {
    match event {
        Event::MouseDown {
            button, modifiers, ..
        } => Event::MouseDown {
            x,
            y,
            button,
            modifiers,
        },
        Event::MouseUp {
            button, modifiers, ..
        } => Event::MouseUp {
            x,
            y,
            button,
            modifiers,
        },
        Event::MouseMove { modifiers, .. } => Event::MouseMove { x, y, modifiers },
        Event::MouseDoubleClick {
            button, modifiers, ..
        } => Event::MouseDoubleClick {
            x,
            y,
            button,
            modifiers,
        },
        Event::MouseWheel {
            delta,
            horizontal,
            modifiers,
            ..
        } => Event::MouseWheel {
            delta,
            horizontal,
            x,
            y,
            modifiers,
        },
        other => other,
    }
}

impl Shared {
    /// Delivers a positional `event` as a window system would: to the captured
    /// node, or the topmost node under the pointer. A move over a new node
    /// first tells the old one it left, and a press first focuses its node.
    pub(super) fn pointer(&self, window: WindowId, event: Event) -> bool {
        let Some((x, y)) = event.position() else {
            return false;
        };
        // A locked pointer reports relative motion instead of positions, and
        // its node takes the buttons and the wheel.
        let locked = self.locked.get();
        if locked.is_some() && matches!(event, Event::MouseMove { .. }) {
            return false;
        }
        let captured = locked.or(self.captured.get());
        let target = match captured {
            Some(id) => {
                let origin = self.scene.borrow().absolute_bounds(id);
                origin.map(|abs| (id, x - abs.left, y - abs.top))
            }
            None => self.scene.borrow().hit_test(window, x, y),
        };
        if matches!(event, Event::MouseMove { .. }) && captured.is_none() {
            self.set_hover(window, target.map(|(id, _, _)| id));
        }
        let is_press = matches!(
            event,
            Event::MouseDown { .. } | Event::MouseDoubleClick { .. }
        );
        let Some((id, local_x, local_y)) = target else {
            if is_press && captured.is_none() {
                self.dismiss_popup_focus(window);
            }
            return false;
        };
        if is_press {
            self.focus_on_press(window, id);
        }
        self.deliver(window, id, &translate(event, local_x, local_y))
    }

    /// The pointer left the window.
    pub(super) fn leave(&self, window: WindowId) {
        if let Some(state) = self.windows.borrow_mut().get_mut(&window.raw()) {
            state.tracking = false;
        }
        if self.captured.get().is_none() {
            self.set_hover(window, None);
        }
    }

    /// The pointer is now over `target` (or nothing); the node it was over
    /// before is told it left.
    fn set_hover(&self, window: WindowId, target: Option<WidgetId>) {
        let previous = self
            .windows
            .borrow_mut()
            .get_mut(&window.raw())
            .and_then(|state| std::mem::replace(&mut state.hover, target));
        let Some(previous) = previous.filter(|previous| Some(*previous) != target) else {
            return;
        };
        if self.scene.borrow().get(previous).is_some() {
            self.deliver(window, previous, &Event::MouseLeave);
        }
    }

    /// Moves the keyboard focus to `id`, telling the old holder and the new.
    pub(super) fn focus_node(&self, id: WidgetId) {
        let Some(window) = self.window_of(id) else {
            return;
        };
        let previous = self.swap_focus(window, Some(id));
        if let Some(previous) = previous.filter(|previous| *previous != id) {
            self.deliver(window, previous, &Event::KillFocus);
        }
        self.deliver(window, id, &Event::SetFocus);
    }

    /// A press focuses the node it lands on before the node sees it; a node
    /// that already has the focus is not told again.
    fn focus_on_press(&self, window: WindowId, id: WidgetId) {
        let previous = self.swap_focus(window, Some(id));
        if let Some(previous) = previous.filter(|previous| *previous != id) {
            self.deliver(window, previous, &Event::KillFocus);
        }
        if previous != Some(id) {
            self.deliver(window, id, &Event::SetFocus);
        }
    }

    /// A press on bare window takes the focus from a popup that holds it, as a
    /// press outside the popup dismisses it. Any other holder keeps it.
    fn dismiss_popup_focus(&self, window: WindowId) {
        let Some(holder) = self.focus_holder(window) else {
            return;
        };
        if self
            .scene
            .borrow()
            .get(holder)
            .is_some_and(|node| node.popup)
        {
            self.swap_focus(window, None);
            self.deliver(window, holder, &Event::KillFocus);
        }
    }

    pub(super) fn focus_holder(&self, window: WindowId) -> Option<WidgetId> {
        self.windows.borrow().get(&window.raw())?.focused
    }

    fn swap_focus(&self, window: WindowId, next: Option<WidgetId>) -> Option<WidgetId> {
        let mut windows = self.windows.borrow_mut();
        std::mem::replace(&mut windows.get_mut(&window.raw())?.focused, next)
    }

    /// Forgets `id` as hovered, focused or captured, as it is destroyed.
    pub(super) fn forget(&self, window: WindowId, id: WidgetId) {
        if let Some(state) = self.windows.borrow_mut().get_mut(&window.raw()) {
            if state.hover == Some(id) {
                state.hover = None;
            }
            if state.focused == Some(id) {
                state.focused = None;
            }
        }
        if self.captured.get() == Some(id) {
            self.captured.set(None);
        }
        if self.locked.get() == Some(id) {
            self.unlock_pointer(false);
        }
    }
}
