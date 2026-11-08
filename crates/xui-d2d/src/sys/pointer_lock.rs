//! Pointer lock: the cursor hidden and pinned to one point while raw mouse
//! motion arrives as `WM_INPUT`, which is unscaled and unclamped, as mouse
//! look needs.

use windows::Win32::Foundation::{HWND, POINT, RECT};
use windows::Win32::Graphics::Gdi::ClientToScreen;
use windows::Win32::UI::Input::{
    GetRawInputData, HRAWINPUT, RAWINPUT, RAWINPUTDEVICE, RAWINPUTHEADER, RID_INPUT, RIDEV_REMOVE,
    RIM_TYPEMOUSE, RegisterRawInputDevices,
};
use windows::Win32::UI::WindowsAndMessaging::{ClipCursor, GetClientRect, SetCursor};

use super::window::Hwnd;

/// `MOUSE_MOVE_ABSOLUTE` (winuser.h): the motion is a position, not a delta.
const MOUSE_MOVE_ABSOLUTE: u16 = 0x01;

/// The generic-desktop usage page and mouse usage (hidusage.h), which the
/// `windows` crate keeps behind another feature.
const USAGE_PAGE_GENERIC: u16 = 0x01;
const USAGE_MOUSE: u16 = 0x02;

fn mouse_device(
    hwnd: HWND,
    flags: windows::Win32::UI::Input::RAWINPUTDEVICE_FLAGS,
) -> RAWINPUTDEVICE {
    RAWINPUTDEVICE {
        usUsagePage: USAGE_PAGE_GENERIC,
        usUsage: USAGE_MOUSE,
        dwFlags: flags,
        // A device being removed takes no target window.
        hwndTarget: if flags == RIDEV_REMOVE {
            HWND::default()
        } else {
            hwnd
        },
    }
}

/// Starts raw mouse input to `hwnd`, pins the cursor to the middle of its
/// client area and hides it; whether the raw device could be registered.
pub(crate) fn lock(hwnd: Hwnd) -> bool {
    let device = [mouse_device(hwnd.raw(), Default::default())];
    // SAFETY: a one-element array of a fully initialised struct, and its size.
    let registered =
        unsafe { RegisterRawInputDevices(&device, size_of::<RAWINPUTDEVICE>() as u32).is_ok() };
    if registered {
        pin(hwnd);
        hide_cursor();
    }
    registered
}

/// Pins the cursor to the middle of `hwnd`'s client area, again after the
/// window moved or changed size while locked.
pub(crate) fn pin(hwnd: Hwnd) {
    let mut client = RECT::default();
    // SAFETY: valid out-pointers; the clip is a one-pixel rectangle.
    unsafe {
        if GetClientRect(hwnd.raw(), &mut client).is_err() {
            return;
        }
        let mut middle = POINT {
            x: (client.left + client.right) / 2,
            y: (client.top + client.bottom) / 2,
        };
        let _ = ClientToScreen(hwnd.raw(), &mut middle);
        let pin = RECT {
            left: middle.x,
            top: middle.y,
            right: middle.x + 1,
            bottom: middle.y + 1,
        };
        let _ = ClipCursor(Some(&pin));
    }
}

/// Ends raw mouse input and frees the cursor. The registration is the
/// process's for the mouse, so one the application made itself ends too.
pub(crate) fn unlock() {
    let device = [mouse_device(HWND::default(), RIDEV_REMOVE)];
    // SAFETY: as in `lock`; releasing the clip is always allowed.
    unsafe {
        let _ = RegisterRawInputDevices(&device, size_of::<RAWINPUTDEVICE>() as u32);
        let _ = ClipCursor(None);
    }
}

/// Hides the cursor (a `WM_SETCURSOR` answer while locked).
pub(crate) fn hide_cursor() {
    // SAFETY: selecting no cursor.
    unsafe { SetCursor(None) };
}

/// The relative mouse motion a `WM_INPUT` message reports, if it is one.
pub(crate) fn raw_delta(lparam: isize) -> Option<(i32, i32)> {
    let handle = HRAWINPUT(lparam as *mut _);
    let mut input = RAWINPUT::default();
    let mut size = size_of::<RAWINPUT>() as u32;
    // SAFETY: `input` is a writable RAWINPUT of `size` bytes, and the header
    // size is that of the struct.
    let read = unsafe {
        GetRawInputData(
            handle,
            RID_INPUT,
            Some(&mut input as *mut RAWINPUT as *mut _),
            &mut size,
            size_of::<RAWINPUTHEADER>() as u32,
        )
    };
    if read == u32::MAX || input.header.dwType != RIM_TYPEMOUSE.0 {
        return None;
    }
    // SAFETY: the header says this is mouse data, so the `mouse` arm is the
    // one the system filled in.
    let mouse = unsafe { input.data.mouse };
    (mouse.usFlags.0 & MOUSE_MOVE_ABSOLUTE == 0).then_some((mouse.lLastX, mouse.lLastY))
}
