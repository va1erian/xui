//! The message loops: the main one, a modal window's nested one, and the quit
//! that ends them.

use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetMessageW, MSG, PostQuitMessage, TranslateMessage,
};

/// Pumps messages until `WM_QUIT`, returning its exit code.
pub(crate) fn run_loop() -> i32 {
    let mut msg = MSG::default();
    // SAFETY: the standard message loop on the thread that owns the windows.
    unsafe {
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
    msg.wParam.0 as i32
}

/// Ends [`run_loop`] with `code`.
pub(crate) fn post_quit(code: i32) {
    // SAFETY: posts WM_QUIT to this thread's queue.
    unsafe { PostQuitMessage(code) };
}

/// Pumps messages while `keep_going` holds, for a modal window's own loop. A
/// `WM_QUIT` ends it and is re-posted so the outer loop sees it too.
pub(crate) fn run_nested(keep_going: impl Fn() -> bool) {
    let mut msg = MSG::default();
    // SAFETY: the standard message loop on the thread that owns the windows.
    unsafe {
        while keep_going() {
            if !GetMessageW(&mut msg, None, 0, 0).as_bool() {
                PostQuitMessage(msg.wParam.0 as i32);
                break;
            }
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}
