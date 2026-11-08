//! Native child windows a node can own for a GPU API to present into: the
//! window class, the placement calls, and the handles handed to the holder.

use std::num::NonZeroIsize;
use std::sync::atomic::{AtomicBool, Ordering};

use raw_window_handle::{
    DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, RawWindowHandle,
    Win32WindowHandle, WindowHandle,
};
use windows::Win32::Foundation::{LPARAM, POINT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    GetCursorPos, GetParent, PostMessageW, SWP_HIDEWINDOW, SWP_NOACTIVATE, SWP_NOZORDER,
    SWP_SHOWWINDOW, SetWindowPos, WM_APP, WS_CHILD, WS_CLIPCHILDREN, WS_CLIPSIBLINGS,
    WindowFromPoint,
};
use windows::core::w;
use xui_core::Rect;

use super::window::{self, Handler, Hwnd};

/// Posted to a surface window to destroy it, from any thread: `DestroyWindow`
/// works only on the thread that made the window.
pub(crate) const WM_SURFACE_DESTROY: u32 = WM_APP + 2;

/// A surface window as its holder sees it: the handles of a live `HWND`.
///
/// The backend refuses the handles once the node gives the window up
/// ([`SurfaceWindow::orphan`]), and destroys the window when the last holder
/// drops it, unless its parent already took it down ([`SurfaceWindow::gone`]).
pub(crate) struct SurfaceWindow {
    hwnd: Hwnd,
    hinstance: Option<NonZeroIsize>,
    /// Cleared when the node gives the window up, or the parent goes.
    alive: AtomicBool,
    /// Set when the parent was destroyed, and the window with it.
    destroyed: AtomicBool,
}

impl SurfaceWindow {
    /// Wraps `hwnd`, a window made by [`create`].
    pub(crate) fn new(hwnd: Hwnd) -> SurfaceWindow {
        // SAFETY: a plain module-handle query.
        let hinstance = unsafe { GetModuleHandleW(None) }
            .ok()
            .and_then(|module| NonZeroIsize::new(module.0 as isize));
        SurfaceWindow {
            hwnd,
            hinstance,
            alive: AtomicBool::new(true),
            destroyed: AtomicBool::new(false),
        }
    }

    pub(crate) fn hwnd(&self) -> Hwnd {
        self.hwnd
    }

    /// Says the node gave the window up: its handles are refused from now.
    pub(crate) fn orphan(&self) {
        self.alive.store(false, Ordering::Release);
    }

    /// Says the window no longer exists, its parent having been destroyed.
    pub(crate) fn gone(&self) {
        self.destroyed.store(true, Ordering::Release);
        self.orphan();
    }

    fn live(&self) -> Result<(), HandleError> {
        if self.alive.load(Ordering::Acquire) {
            Ok(())
        } else {
            Err(HandleError::Unavailable)
        }
    }
}

impl Drop for SurfaceWindow {
    fn drop(&mut self) {
        if !self.destroyed.load(Ordering::Acquire) {
            // SAFETY: `PostMessageW` is documented thread-safe; a window that
            // is gone makes it fail harmlessly.
            let _ = unsafe {
                PostMessageW(
                    Some(self.hwnd.raw()),
                    WM_SURFACE_DESTROY,
                    WPARAM(0),
                    LPARAM(0),
                )
            };
        }
    }
}

impl HasWindowHandle for SurfaceWindow {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        self.live()?;
        let value = NonZeroIsize::new(self.hwnd.value()).ok_or(HandleError::Unavailable)?;
        let mut handle = Win32WindowHandle::new(value);
        handle.hinstance = self.hinstance;
        // SAFETY: `value` is the `HWND` of a window this backend made and has
        // not given up (`alive`), so it named a live window when this was
        // called. A parent closed on the UI thread while another thread still
        // uses the borrow leaves that thread an identifier the OS rejects
        // (presents fail), never memory of ours.
        Ok(unsafe { WindowHandle::borrow_raw(RawWindowHandle::Win32(handle)) })
    }
}

impl HasDisplayHandle for SurfaceWindow {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        self.live()?;
        Ok(DisplayHandle::windows())
    }
}

/// Makes the hidden child window of `parent` that a surface node presents
/// into; its messages go to `handler`.
pub(crate) fn create(parent: Hwnd, handler: Handler) -> Option<Hwnd> {
    window::register_class_named(w!("xui_d2d_surface"));
    window::create_with(
        w!("xui_d2d_surface"),
        "",
        WS_CHILD | WS_CLIPSIBLINGS | WS_CLIPCHILDREN,
        Some(parent),
        handler,
    )
}

/// Puts the window at `rect` (its parent's client pixels) and shows or hides
/// it, each only when given. Never activates it.
pub(crate) fn place(hwnd: Hwnd, rect: Option<Rect>, show: Option<bool>) {
    let r = rect.unwrap_or_default();
    let mut flags = SWP_NOACTIVATE | SWP_NOZORDER;
    if rect.is_none() {
        flags |= windows::Win32::UI::WindowsAndMessaging::SWP_NOMOVE
            | windows::Win32::UI::WindowsAndMessaging::SWP_NOSIZE;
    }
    match show {
        Some(true) => flags |= SWP_SHOWWINDOW,
        Some(false) => flags |= SWP_HIDEWINDOW,
        None => {}
    }
    // SAFETY: a plain move of a child window this thread made.
    let _ = unsafe {
        SetWindowPos(
            hwnd.raw(),
            None,
            r.left,
            r.top,
            r.width().max(1),
            r.height().max(1),
            flags,
        )
    };
}

/// Whether the pointer is over a child window of `parent` (a surface).
pub(crate) fn pointer_over_child(parent: Hwnd) -> bool {
    let mut at = POINT::default();
    // SAFETY: `at` is a valid out-pointer; the lookups only read.
    unsafe {
        GetCursorPos(&mut at).is_ok() && {
            let under = WindowFromPoint(at);
            under != parent.raw() && GetParent(under).is_ok_and(|p| p == parent.raw())
        }
    }
}

/// Whether the pointer is over `parent` or one of its child windows, and not
/// over another window (another application's, or one of ours such as a menu).
pub(crate) fn pointer_over(parent: Hwnd) -> bool {
    let mut at = POINT::default();
    // SAFETY: `at` is a valid out-pointer; the lookups only read.
    unsafe {
        GetCursorPos(&mut at).is_ok() && {
            let under = WindowFromPoint(at);
            under == parent.raw() || GetParent(under).is_ok_and(|p| p == parent.raw())
        }
    }
}
