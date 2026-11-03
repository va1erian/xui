//! Top-level windows: the window class, the message trampoline and the
//! operations the backend needs on a window.

use std::rc::Rc;

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Dwm::{DWMWA_USE_IMMERSIVE_DARK_MODE, DwmSetWindowAttribute};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, EndPaint, InvalidateRect, PAINTSTRUCT, ScreenToClient, UpdateWindow,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::{
    AdjustWindowRectExForDpi, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, GetDpiForWindow,
    SetProcessDpiAwarenessContext,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    EnableWindow, GetKeyState, ReleaseCapture, SetCapture, SetFocus, TME_LEAVE, TRACKMOUSEEVENT,
    TrackMouseEvent, VK_CONTROL, VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CREATESTRUCTW, CS_DBLCLKS, CW_USEDEFAULT, CreateWindowExW, DefWindowProcW, DestroyWindow,
    DispatchMessageW, GWLP_USERDATA, GetClientRect, GetMessageW, GetWindowLongPtrW, HMENU,
    IDC_ARROW, IDC_HAND, IDC_IBEAM, IDC_SIZENESW, IDC_SIZENS, IDC_SIZENWSE, IDC_SIZEWE, IDC_WAIT,
    IsZoomed, KillTimer, LoadCursorW, MSG, PostMessageW, PostQuitMessage, RegisterClassExW,
    SW_MAXIMIZE, SW_MINIMIZE, SW_RESTORE, SW_SHOW, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOZORDER,
    SetCursor, SetTimer, SetWindowLongPtrW, SetWindowPos, SetWindowTextW, ShowWindow,
    TranslateMessage, WINDOW_EX_STYLE, WM_APP, WM_NCCREATE, WM_NCDESTROY, WNDCLASSEXW,
    WS_MAXIMIZEBOX, WS_OVERLAPPEDWINDOW, WS_THICKFRAME,
};
use windows::core::{HSTRING, PCWSTR, w};
use xui_core::backend::Cursor;
use xui_core::message::Modifiers;
use xui_core::{Dip, Rect};

/// A native window handle; opaque outside the `sys` module.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct Hwnd(isize);

impl Hwnd {
    pub(super) fn raw(self) -> HWND {
        HWND(self.0 as *mut _)
    }
}

/// One window message, as the window procedure received it.
#[derive(Clone, Copy)]
pub(crate) struct Msg {
    pub(crate) id: u32,
    pub(crate) wparam: usize,
    pub(crate) lparam: isize,
}

/// A window's message handler: `Some(result)` when it handled the message,
/// `None` to leave it to the default window procedure.
pub(crate) type Handler = Rc<dyn Fn(Hwnd, Msg) -> Option<isize>>;

/// The message `post_wake` posts, handled as `Event::Wake`.
pub(crate) const WM_WAKE: u32 = WM_APP + 1;

const CLASS: PCWSTR = w!("xui_d2d_window");

/// Makes the process per-monitor DPI aware (v2). Idempotent; a second call
/// fails harmlessly.
pub(crate) fn init() {
    // SAFETY: a plain call with a documented constant; it only changes the
    // process's DPI awareness and is allowed to fail once it is set.
    let _ = unsafe { SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if msg == WM_NCCREATE {
        // SAFETY: for WM_NCCREATE, `lparam` points to the CREATESTRUCTW whose
        // `lpCreateParams` is the boxed handler `create` passed.
        let params = unsafe { (*(lparam.0 as *const CREATESTRUCTW)).lpCreateParams };
        // SAFETY: `hwnd` is the window being created.
        unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, params as isize) };
    }
    // SAFETY: the user data is either 0 or the `Box<Handler>` stored above,
    // which lives until WM_NCDESTROY below.
    let slot = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *const Handler;
    if slot.is_null() {
        // SAFETY: forwarding the message this procedure received.
        return unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) };
    }
    // SAFETY: `slot` is live (see above); the clone keeps the handler alive
    // across a re-entrant destroy of this window.
    let handler = unsafe { (*slot).clone() };
    let result = handler(
        Hwnd(hwnd.0 as isize),
        Msg {
            id: msg,
            wparam: wparam.0,
            lparam: lparam.0,
        },
    );
    if msg == WM_NCDESTROY {
        // SAFETY: the last message for this window; nothing reads the slot
        // once the user data is cleared, and the box came from `create`.
        unsafe {
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
            drop(Box::from_raw(slot as *mut Handler));
        }
    }
    match result {
        Some(value) => LRESULT(value),
        // SAFETY: forwarding the message this procedure received.
        None => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

fn register_class() {
    // SAFETY: registers a class whose procedure is `wndproc`; registering the
    // same class twice fails harmlessly, which is what makes this idempotent.
    unsafe {
        let Ok(instance) = GetModuleHandleW(None) else {
            return;
        };
        let class = WNDCLASSEXW {
            cbSize: size_of::<WNDCLASSEXW>() as u32,
            style: CS_DBLCLKS,
            lpfnWndProc: Some(wndproc),
            hInstance: instance.into(),
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            lpszClassName: CLASS,
            ..Default::default()
        };
        RegisterClassExW(&class);
    }
}

/// Creates a hidden top-level window whose client area is `width` x `height`
/// design units at the DPI of the monitor it lands on.
pub(crate) fn create(
    title: &str,
    width: Dip,
    height: Dip,
    resizable: bool,
    handler: Handler,
) -> Option<Hwnd> {
    register_class();
    let mut style = WS_OVERLAPPEDWINDOW;
    if !resizable {
        style &= !(WS_THICKFRAME | WS_MAXIMIZEBOX);
    }
    let params = Box::into_raw(Box::new(handler));
    // SAFETY: creates a window of the class registered above. `params` is the
    // boxed handler `wndproc` stores on WM_NCCREATE and frees on WM_NCDESTROY.
    let created = unsafe {
        let instance = GetModuleHandleW(None).ok()?;
        CreateWindowExW(
            WINDOW_EX_STYLE(0),
            CLASS,
            &HSTRING::from(title),
            style,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            None,
            Some(HMENU::default()),
            Some(instance.into()),
            Some(params as *const _),
        )
    };
    let Ok(hwnd) = created else {
        // SAFETY: creation failed, so WM_NCCREATE never stored the box (or
        // WM_NCDESTROY already freed it when it did); a failed
        // `CreateWindowExW` before WM_NCCREATE is the case reclaimed here.
        unsafe { drop(Box::from_raw(params)) };
        return None;
    };
    let hwnd = Hwnd(hwnd.0 as isize);
    let dpi = dpi(hwnd);
    let mut frame = RECT {
        left: 0,
        top: 0,
        right: width.to_px(dpi).value(),
        bottom: height.to_px(dpi).value(),
    };
    // SAFETY: `frame` is a valid RECT and `hwnd` was just created.
    unsafe {
        let _ = AdjustWindowRectExForDpi(&mut frame, style, false, WINDOW_EX_STYLE(0), dpi);
        let _ = SetWindowPos(
            hwnd.raw(),
            None,
            0,
            0,
            frame.right - frame.left,
            frame.bottom - frame.top,
            SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
        );
    }
    Some(hwnd)
}

/// The window's DPI.
pub(crate) fn dpi(hwnd: Hwnd) -> u32 {
    // SAFETY: a query on a window handle; an invalid handle returns 0.
    match unsafe { GetDpiForWindow(hwnd.raw()) } {
        0 => 96,
        dpi => dpi,
    }
}

/// The client area's size in device pixels.
pub(crate) fn client_size(hwnd: Hwnd) -> (i32, i32) {
    let mut rect = RECT::default();
    // SAFETY: `rect` is a valid out-pointer.
    let _ = unsafe { GetClientRect(hwnd.raw(), &mut rect) };
    (rect.right - rect.left, rect.bottom - rect.top)
}

/// Destroys the window; its handler receives `WM_NCDESTROY` and is dropped.
pub(crate) fn destroy(hwnd: Hwnd) {
    // SAFETY: destroying a window this thread created.
    let _ = unsafe { DestroyWindow(hwnd.raw()) };
}

/// Shows the window and paints it once.
pub(crate) fn show(hwnd: Hwnd) {
    // SAFETY: plain window calls on a handle this crate created.
    unsafe {
        let _ = ShowWindow(hwnd.raw(), SW_SHOW);
        let _ = UpdateWindow(hwnd.raw());
    }
}

/// Retitles the window.
pub(crate) fn set_title(hwnd: Hwnd, title: &str) {
    // SAFETY: the HSTRING outlives the call.
    let _ = unsafe { SetWindowTextW(hwnd.raw(), &HSTRING::from(title)) };
}

/// Marks `rect` (client pixels) as needing a repaint, or the whole client
/// area when `None`.
pub(crate) fn invalidate(hwnd: Hwnd, rect: Option<Rect>) {
    let native = rect.map(|r| RECT {
        left: r.left,
        top: r.top,
        right: r.right,
        bottom: r.bottom,
    });
    // SAFETY: the optional rect pointer is valid for the call.
    let _ = unsafe {
        InvalidateRect(
            Some(hwnd.raw()),
            native.as_ref().map(|r| r as *const _),
            false,
        )
    };
}

/// Starts a repeating timer; `id` must be non-zero.
pub(crate) fn set_timer(hwnd: Hwnd, id: usize, millis: u32) -> bool {
    // SAFETY: a timer on a window of this thread, with no callback.
    unsafe { SetTimer(Some(hwnd.raw()), id, millis.max(1), None) != 0 }
}

/// Stops a timer started with [`set_timer`].
pub(crate) fn kill_timer(hwnd: Hwnd, id: usize) {
    // SAFETY: killing an unknown timer fails harmlessly.
    let _ = unsafe { KillTimer(Some(hwnd.raw()), id) };
}

/// Posts [`WM_WAKE`] to the window; safe from any thread.
pub(crate) fn post_wake(hwnd: Hwnd) {
    // SAFETY: `PostMessageW` is documented thread-safe; a dead handle fails.
    let _ = unsafe { PostMessageW(Some(hwnd.raw()), WM_WAKE, WPARAM(0), LPARAM(0)) };
}

/// Asks to be told when the pointer leaves the client area.
pub(crate) fn track_leave(hwnd: Hwnd) {
    let mut track = TRACKMOUSEEVENT {
        cbSize: size_of::<TRACKMOUSEEVENT>() as u32,
        dwFlags: TME_LEAVE,
        hwndTrack: hwnd.raw(),
        dwHoverTime: 0,
    };
    // SAFETY: `track` is a fully initialised struct.
    let _ = unsafe { TrackMouseEvent(&mut track) };
}

/// Captures the mouse for the window, so moves and releases arrive outside it.
pub(crate) fn capture(hwnd: Hwnd) {
    // SAFETY: capture for a window of this thread.
    unsafe { SetCapture(hwnd.raw()) };
}

/// Releases a mouse capture.
pub(crate) fn release_capture() {
    // SAFETY: releasing capture is always allowed.
    let _ = unsafe { ReleaseCapture() };
}

/// Gives the window the keyboard focus.
pub(crate) fn focus(hwnd: Hwnd) {
    // SAFETY: focus for a window of this thread.
    let _ = unsafe { SetFocus(Some(hwnd.raw())) };
}

/// Enables or disables the whole window.
pub(crate) fn set_enabled(hwnd: Hwnd, enabled: bool) {
    // SAFETY: plain window call.
    let _ = unsafe { EnableWindow(hwnd.raw(), enabled) };
}

/// Minimizes the window.
pub(crate) fn minimize(hwnd: Hwnd) {
    // SAFETY: plain window call.
    let _ = unsafe { ShowWindow(hwnd.raw(), SW_MINIMIZE) };
}

/// Maximizes the window, or restores it when it is maximized.
pub(crate) fn toggle_maximize(hwnd: Hwnd) {
    let command = if is_maximized(hwnd) {
        SW_RESTORE
    } else {
        SW_MAXIMIZE
    };
    // SAFETY: plain window call.
    let _ = unsafe { ShowWindow(hwnd.raw(), command) };
}

/// Whether the window is maximized.
pub(crate) fn is_maximized(hwnd: Hwnd) -> bool {
    // SAFETY: a query on a window handle.
    unsafe { IsZoomed(hwnd.raw()).as_bool() }
}

/// Matches the title bar to the theme (Windows 10 1809+; ignored elsewhere).
pub(crate) fn set_dark_title_bar(hwnd: Hwnd, dark: bool) {
    let value = i32::from(dark);
    // SAFETY: the attribute value is a 4-byte BOOL that outlives the call.
    let _ = unsafe {
        DwmSetWindowAttribute(
            hwnd.raw(),
            DWMWA_USE_IMMERSIVE_DARK_MODE,
            &value as *const i32 as *const _,
            size_of::<i32>() as u32,
        )
    };
}

/// Shows the system cursor that matches `cursor`.
pub(crate) fn set_cursor(cursor: Cursor) {
    let id = match cursor {
        Cursor::Default => IDC_ARROW,
        Cursor::Hand => IDC_HAND,
        Cursor::Text => IDC_IBEAM,
        Cursor::SizeHorizontal => IDC_SIZEWE,
        Cursor::SizeVertical => IDC_SIZENS,
        Cursor::SizeNwSe => IDC_SIZENWSE,
        Cursor::SizeNeSw => IDC_SIZENESW,
        Cursor::Busy => IDC_WAIT,
    };
    // SAFETY: loading a shared system cursor and selecting it.
    unsafe {
        if let Ok(handle) = LoadCursorW(None, id) {
            SetCursor(Some(handle));
        }
    }
}

/// Converts a point from screen to client coordinates.
pub(crate) fn screen_to_client(hwnd: Hwnd, x: i32, y: i32) -> (i32, i32) {
    let mut point = POINT { x, y };
    // SAFETY: `point` is a valid in/out pointer.
    let _ = unsafe { ScreenToClient(hwnd.raw(), &mut point) };
    (point.x, point.y)
}

/// The Ctrl, Shift, Alt and Windows keys held now.
pub(crate) fn modifiers() -> Modifiers {
    let down = |key: i32| {
        // SAFETY: a keyboard state query.
        let state = unsafe { GetKeyState(key) };
        state < 0
    };
    Modifiers {
        ctrl: down(i32::from(VK_CONTROL.0)),
        shift: down(i32::from(VK_SHIFT.0)),
        alt: down(i32::from(VK_MENU.0)),
        win: down(i32::from(VK_LWIN.0)) || down(i32::from(VK_RWIN.0)),
    }
}

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

/// Runs `draw` with the rectangle that needs repainting, inside a
/// `BeginPaint`/`EndPaint` pair that also marks the window valid.
pub(crate) fn paint(hwnd: Hwnd, draw: impl FnOnce(Rect)) {
    let mut ps = PAINTSTRUCT::default();
    // SAFETY: `ps` is a valid out-struct; `EndPaint` below closes this call.
    unsafe { BeginPaint(hwnd.raw(), &mut ps) };
    let dirty = Rect::new(
        ps.rcPaint.left,
        ps.rcPaint.top,
        ps.rcPaint.right,
        ps.rcPaint.bottom,
    );
    draw(dirty);
    // SAFETY: closes the `BeginPaint` above with the same struct.
    let _ = unsafe { EndPaint(hwnd.raw(), &ps) };
}

/// Moves the window to the rectangle Windows suggests in `WM_DPICHANGED`'s
/// `lparam`, and returns it.
pub(crate) fn apply_suggested_rect(hwnd: Hwnd, lparam: isize) -> Rect {
    // SAFETY: for WM_DPICHANGED, `lparam` points to a RECT valid for the
    // duration of the message.
    let suggested = unsafe { *(lparam as *const RECT) };
    // SAFETY: a plain window move on a window of this thread.
    let _ = unsafe {
        SetWindowPos(
            hwnd.raw(),
            None,
            suggested.left,
            suggested.top,
            suggested.right - suggested.left,
            suggested.bottom - suggested.top,
            SWP_NOZORDER | SWP_NOACTIVATE,
        )
    };
    Rect::new(
        suggested.left,
        suggested.top,
        suggested.right,
        suggested.bottom,
    )
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
