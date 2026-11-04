//! The hidden message-only window a thread owns for operations that need a real
//! window handle as owner.

use windows::Win32::Foundation::HWND;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DestroyWindow, HWND_MESSAGE, WINDOW_EX_STYLE, WINDOW_STYLE,
};
use windows::core::w;

use super::window::{CLASS, register_class};

/// A hidden message-only window of the calling thread, for the operations that
/// need a real window handle as their owner (the clipboard: opening it with no
/// window lets several threads open it at once, and `EmptyClipboard` then frees
/// data another is reading).
pub(super) struct MessageWindow(HWND);

impl MessageWindow {
    pub(super) fn new() -> Option<MessageWindow> {
        register_class();
        // SAFETY: creates a message-only window of the registered class; with
        // no creation parameter `wndproc` finds no slot and defers to
        // `DefWindowProcW`.
        let created = unsafe {
            let instance = GetModuleHandleW(None).ok()?;
            CreateWindowExW(
                WINDOW_EX_STYLE(0),
                CLASS,
                w!(""),
                WINDOW_STYLE(0),
                0,
                0,
                0,
                0,
                Some(HWND_MESSAGE),
                None,
                Some(instance.into()),
                None,
            )
        };
        created.ok().map(MessageWindow)
    }

    pub(super) fn raw(&self) -> HWND {
        self.0
    }
}

impl Drop for MessageWindow {
    fn drop(&mut self) {
        // SAFETY: destroying a window this thread created.
        let _ = unsafe { DestroyWindow(self.0) };
    }
}
