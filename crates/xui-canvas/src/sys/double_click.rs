//! The system's double-click interval and distance.
//!
//! `winit` does not report double-clicks, so the canvas backend synthesizes
//! them from the press sequence and needs the platform's thresholds. Windows
//! exposes the user's configured values as `GetDoubleClickTime` and
//! `SM_CXDOUBLECLK`/`SM_CYDOUBLECLK` (`Winuser.h`); every other platform has no
//! such query here and uses the documented fallback.

use std::time::Duration;

/// The double-click interval used when the platform has no query, in
/// milliseconds.
pub(crate) const FALLBACK_TIME: Duration = Duration::from_millis(500);

/// The double-click distance used when the platform has no query, in device
/// pixels.
pub(crate) const FALLBACK_DISTANCE: i32 = 4;

/// The platform's double-click interval and distance (device pixels).
///
/// On Windows these are the user's configured values, falling back when the
/// system reports none; elsewhere they are [`FALLBACK_TIME`] and
/// [`FALLBACK_DISTANCE`], since `winit` exposes no query.
#[cfg(windows)]
pub(crate) fn system() -> (Duration, i32) {
    use windows::Win32::UI::Input::KeyboardAndMouse::GetDoubleClickTime;
    use windows::Win32::UI::WindowsAndMessaging::{
        GetSystemMetrics, SM_CXDOUBLECLK, SM_CYDOUBLECLK,
    };

    // SAFETY: both entry points take no pointers and return plain integers;
    // they have no preconditions and cannot fail for these indices.
    let milliseconds = unsafe { GetDoubleClickTime() };
    let (wide, high) = unsafe {
        (
            GetSystemMetrics(SM_CXDOUBLECLK),
            GetSystemMetrics(SM_CYDOUBLECLK),
        )
    };
    let time = if milliseconds == 0 {
        FALLBACK_TIME
    } else {
        Duration::from_millis(u64::from(milliseconds))
    };
    // Win32 compares against a rectangle of `SM_CXDOUBLECLK` by
    // `SM_CYDOUBLECLK`; a single distance keeps the common case, where the two
    // are equal, and stays permissive when they are not.
    let distance = wide.max(high);
    let distance = if distance > 0 {
        distance
    } else {
        FALLBACK_DISTANCE
    };
    (time, distance)
}

/// The documented fallback, for platforms whose `winit` backend exposes no
/// double-click query.
#[cfg(not(windows))]
pub(crate) fn system() -> (Duration, i32) {
    (FALLBACK_TIME, FALLBACK_DISTANCE)
}
