//! The message loops: the main one, a modal window's nested one, and the quit
//! that ends them. While a frame is pending they also wait on the frame timer.

use std::time::Duration;

use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetMessageW, MSG, PM_REMOVE, PeekMessageW, PostQuitMessage, TranslateMessage,
    WM_QUIT,
};

use super::frame_timer::{FrameTimer, Woke};

/// The frames the loops tick, kept by the backend.
pub(crate) trait Frames {
    /// The timer to wait on, or `None` where there is none and no frame can
    /// be paced.
    fn timer(&self) -> Option<&FrameTimer>;
    /// How long until the earliest pending frame is due (zero when it is
    /// already), or `None` when no frame is pending.
    fn until_next(&self) -> Option<Duration>;
    /// Delivers every frame that is due, and only those.
    fn deliver_due(&self);
}

/// What one turn of a loop found.
enum Turn {
    Continue,
    Quit(i32),
}

/// Waits for and handles the next message or due frame. With no frame
/// pending it blocks in `GetMessageW`, exactly as a plain message loop does.
fn turn(frames: &dyn Frames) -> Turn {
    let mut msg = MSG::default();
    if let (Some(timer), Some(delay)) = (frames.timer(), frames.until_next()) {
        if timer.wait(delay) == Woke::Timer {
            frames.deliver_due();
            return Turn::Continue;
        }
        // SAFETY: the standard message calls on the thread that owns the windows.
        unsafe {
            while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                if msg.message == WM_QUIT {
                    return Turn::Quit(msg.wParam.0 as i32);
                }
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
        return Turn::Continue;
    }
    // SAFETY: the standard message loop on the thread that owns the windows.
    unsafe {
        if !GetMessageW(&mut msg, None, 0, 0).as_bool() {
            return Turn::Quit(msg.wParam.0 as i32);
        }
        let _ = TranslateMessage(&msg);
        DispatchMessageW(&msg);
    }
    Turn::Continue
}

/// Pumps messages until `WM_QUIT`, returning its exit code.
pub(crate) fn run_loop(frames: &dyn Frames) -> i32 {
    loop {
        if let Turn::Quit(code) = turn(frames) {
            return code;
        }
    }
}

/// Ends [`run_loop`] with `code`.
pub(crate) fn post_quit(code: i32) {
    // SAFETY: posts WM_QUIT to this thread's queue.
    unsafe { PostQuitMessage(code) };
}

/// Pumps messages while `keep_going` holds, for a modal window's own loop. A
/// `WM_QUIT` ends it and is re-posted so the outer loop sees it too. Pending
/// frames keep ticking here.
pub(crate) fn run_nested(frames: &dyn Frames, keep_going: impl Fn() -> bool) {
    while keep_going() {
        if let Turn::Quit(code) = turn(frames) {
            post_quit(code);
            break;
        }
    }
}
