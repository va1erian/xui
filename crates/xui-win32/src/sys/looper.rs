//! The message pump: `GetMessageW`/`TranslateMessage`/`DispatchMessageW`.

use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetMessageW, MSG, PostQuitMessage, TranslateMessage,
};

/// The outcome of pumping one message.
pub(crate) enum Pumped {
    /// A message was retrieved and dispatched.
    Message,
    /// `WM_QUIT` was received; carries the exit code.
    Quit(i32),
    /// `GetMessageW` failed.
    Error,
}

/// Retrieves and dispatches a single message. Blocks until one is available.
pub(crate) fn pump() -> Pumped {
    let mut msg = MSG::default();
    // SAFETY: `msg` is a valid, aligned out-pointer and `GetMessageW` fully
    // initialises it before returning a positive value.
    let result = unsafe { GetMessageW(&mut msg, None, 0, 0) };
    match result.0 {
        0 => return Pumped::Quit(msg.wParam.0 as i32),
        -1 => return Pumped::Error,
        _ => {}
    }
    // SAFETY: the message just retrieved is translated and dispatched under
    // the standard WndProc contract; both calls only read `msg`.
    unsafe {
        let _ = TranslateMessage(&msg);
        DispatchMessageW(&msg);
    }
    Pumped::Message
}

/// Ends the message loop with `code`.
pub(crate) fn post_quit(code: i32) {
    // SAFETY: `PostQuitMessage` takes no pointers and only touches the calling
    // thread's message queue.
    unsafe { PostQuitMessage(code) };
}
