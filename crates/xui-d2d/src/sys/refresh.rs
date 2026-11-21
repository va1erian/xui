//! The refresh rate of the monitor a window is on.

use windows::Win32::Graphics::Gdi::{
    DEVMODEW, ENUM_CURRENT_SETTINGS, ENUM_DISPLAY_SETTINGS_FLAGS, EnumDisplaySettingsExW,
    GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MONITORINFOEXW, MonitorFromWindow,
};
use windows::core::PCWSTR;

use super::window::Hwnd;

/// The refresh rate in hertz of the monitor `hwnd` is mostly on, or 0 when
/// Windows does not say. Windows also reports 0 and 1 for "the hardware
/// default", which says nothing either.
pub(crate) fn refresh_hz(hwnd: Hwnd) -> u32 {
    let mut info = MONITORINFOEXW::default();
    info.monitorInfo.cbSize = size_of::<MONITORINFOEXW>() as u32;
    // SAFETY: `cbSize` announces the extended struct, whose first field is the
    // `MONITORINFO` the function writes through.
    let found = unsafe {
        let monitor = MonitorFromWindow(hwnd.raw(), MONITOR_DEFAULTTONEAREST);
        GetMonitorInfoW(
            monitor,
            &mut info as *mut MONITORINFOEXW as *mut MONITORINFO,
        )
    };
    if !found.as_bool() {
        return 0;
    }
    let mut mode = DEVMODEW {
        dmSize: size_of::<DEVMODEW>() as u16,
        ..Default::default()
    };
    // SAFETY: `szDevice` is the NUL-terminated name just filled in, and `mode`
    // is a writable `DEVMODEW` with its size set.
    let known = unsafe {
        EnumDisplaySettingsExW(
            PCWSTR(info.szDevice.as_ptr()),
            ENUM_CURRENT_SETTINGS,
            &mut mode,
            ENUM_DISPLAY_SETTINGS_FLAGS(0),
        )
    };
    if known.as_bool() {
        mode.dmDisplayFrequency
    } else {
        0
    }
}
