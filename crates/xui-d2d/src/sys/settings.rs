//! Desktop preferences: dark mode and reduced motion.

use windows::Win32::System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_DWORD, RegGetValueW};
use windows::Win32::UI::WindowsAndMessaging::{
    SPI_GETCLIENTAREAANIMATION, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, SystemParametersInfoW,
};
use windows::core::w;

/// Whether the user prefers dark apps (`AppsUseLightTheme` is 0), or `None`
/// when the setting is absent (before Windows 10 1809).
pub(crate) fn prefers_dark() -> Option<bool> {
    let mut value = 1u32;
    let mut size = size_of::<u32>() as u32;
    // SAFETY: `value` and `size` are valid out-pointers sized for a DWORD.
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize"),
            w!("AppsUseLightTheme"),
            RRF_RT_REG_DWORD,
            None,
            Some(&mut value as *mut u32 as *mut _),
            Some(&mut size),
        )
    };
    status.is_ok().then_some(value == 0)
}

/// Whether the user turned client-area animation off (the "animation
/// effects" switch), which xui treats as a request to reduce motion.
pub(crate) fn reduced_motion() -> bool {
    let mut enabled = 1i32;
    // SAFETY: the out-pointer is a 4-byte BOOL, as this query writes.
    let queried = unsafe {
        SystemParametersInfoW(
            SPI_GETCLIENTAREAANIMATION,
            0,
            Some(&mut enabled as *mut i32 as *mut _),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
    };
    queried.is_ok() && enabled == 0
}
