#![forbid(unsafe_code)]

//! The [`Backend::set_theme`](xui_core::backend::Backend::set_theme) body:
//! re-theme the window chrome and every node it owns. Split from
//! `contract.rs` (which delegates to this) so both files stay under the size
//! limit.

use xui_core::Theme;
use xui_core::backend::WindowId;

use super::Win32Backend;
use crate::sys;

impl Win32Backend {
    /// Applies the theme to the window's class background, title bar and
    /// backdrop, then to each node it owns; the body of
    /// [`Backend::set_theme`](xui_core::backend::Backend::set_theme).
    pub(super) fn apply_theme(&self, window: WindowId, theme: &Theme) {
        let hwnd = {
            let windows = self.windows.borrow();
            let Some(entry) = windows.get(&window.raw()) else {
                return;
            };
            entry.theme.set(*theme);
            entry.shared.set_theme(*theme);
            entry.window.hwnd()
        };
        sys::set_class_background(hwnd, theme.background);
        sys::set_titlebar_dark(hwnd, theme.is_dark);
        if crate::window::nc::is_extended(hwnd) {
            sys::apply_extended_colors(hwnd, theme, crate::theme::backdrop_active(hwnd));
        }
        // A native edit's `WM_CTLCOLOREDIT` follows the shared theme live, but
        // its themed `WS_BORDER` frame is painted by a subclass, so re-colour it
        // explicitly. Collect first: the update paints synchronously.
        let edits: Vec<crate::hwnd::Hwnd> = self
            .nodes
            .borrow()
            .values()
            .filter(|node| node.window_id == window)
            .map(|node| node.hwnd)
            .collect();
        for edit in edits {
            sys::edit_edge::set_theme(edit, *theme);
        }
        // Painters read the shared theme live, but they only repaint when
        // asked, so invalidate the whole tree (children included).
        sys::window::redraw_children(hwnd);
        // A popup is a top-level window rather than a child, so the walk above
        // misses it; invalidate each one directly.
        let popups: Vec<crate::hwnd::Hwnd> = self
            .nodes
            .borrow()
            .values()
            .filter(|node| node.is_popup && node.window_id == window)
            .map(|node| node.hwnd)
            .collect();
        for popup in popups {
            sys::window::invalidate(popup);
        }
    }
}
