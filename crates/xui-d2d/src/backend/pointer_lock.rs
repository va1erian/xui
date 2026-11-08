#![forbid(unsafe_code)]

//! Pointer lock: a node gets raw relative motion while the cursor is hidden
//! and pinned, and the lock ends whenever the cursor must come back.

use xui_core::backend::{Event, WidgetId, WindowId};

use super::Shared;
use crate::sys::pointer_lock;

impl Shared {
    /// Hides and pins the cursor for `id`. A lock the system cannot give
    /// (no window, no raw input) never starts.
    pub(super) fn lock_pointer(&self, id: WidgetId) {
        self.unlock_pointer(false);
        let Some(hwnd) = self.window_of(id).and_then(|w| self.hwnd_of(w)) else {
            return;
        };
        if pointer_lock::lock(hwnd) {
            self.locked.set(Some(id));
        }
    }

    /// Gives the cursor back. `notify` sends `PointerLockLost` to the node
    /// that held the lock, for a release it did not ask for.
    pub(super) fn unlock_pointer(&self, notify: bool) {
        let Some(id) = self.locked.take() else {
            return;
        };
        pointer_lock::unlock();
        if notify && let Some(window) = self.window_of(id) {
            self.deliver(window, id, &Event::PointerLockLost);
        }
    }

    /// Ends a lock held in `window`, if any.
    pub(super) fn unlock_window(&self, window: WindowId, notify: bool) {
        if self
            .locked
            .get()
            .is_some_and(|id| self.window_of(id) == Some(window))
        {
            self.unlock_pointer(notify);
        }
    }

    /// Passes the raw motion of a `WM_INPUT` message to the node that holds
    /// the lock in `window`.
    pub(super) fn raw_motion(&self, window: WindowId, lparam: isize) {
        let Some(id) = self
            .locked
            .get()
            .filter(|id| self.window_of(*id) == Some(window))
        else {
            return;
        };
        if let Some((dx, dy)) = pointer_lock::raw_delta(lparam)
            && (dx != 0 || dy != 0)
        {
            self.deliver(window, id, &Event::PointerDelta { dx, dy });
        }
    }
}
