//! Window icons: the executable's default icon on the window class, and a
//! window's own big and small icons at the sizes its DPI calls for.

use std::sync::OnceLock;

use windows::Win32::Foundation::{FALSE, HMODULE, LPARAM, WPARAM};
use windows::Win32::System::LibraryLoader::{EnumResourceNamesW, GetModuleHandleW};
use windows::Win32::UI::HiDpi::GetSystemMetricsForDpi;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateIcon, DestroyIcon, GetSystemMetrics, HICON, ICON_BIG, ICON_SMALL, IMAGE_FLAGS,
    IMAGE_ICON, LR_DEFAULTCOLOR, LR_SHARED, LoadImageW, RT_GROUP_ICON, SM_CXICON, SM_CXSMICON,
    SendMessageW, WM_SETICON,
};
use windows::core::{BOOL, PCWSTR};
use xui_core::backend::WindowIcon;

use super::window::Hwnd;

/// An icon this crate created; destroyed when dropped.
struct Icon(HICON);

impl Drop for Icon {
    fn drop(&mut self) {
        // SAFETY: the handle was created by `LoadImageW` (without
        // `LR_SHARED`) or `CreateIcon`, is owned by this value alone and is
        // destroyed once.
        let _ = unsafe { DestroyIcon(self.0) };
    }
}

/// A window's big (Alt+Tab, task bar) and small (title bar) icons. The window
/// only borrows them, so they are kept until it no longer shows them.
pub(crate) struct WindowIcons {
    big: Icon,
    small: Icon,
}

/// The name of an icon resource: an integer id or a string.
enum Name {
    Id(u16),
    /// Null-terminated UTF-16.
    Text(Vec<u16>),
}

impl Name {
    fn as_pcwstr(&self) -> PCWSTR {
        match self {
            // `MAKEINTRESOURCEW`: an id travels as the pointer value itself.
            Name::Id(id) => PCWSTR(usize::from(*id) as *const u16),
            Name::Text(text) => PCWSTR(text.as_ptr()),
        }
    }
}

/// The executable's first icon group, the one Explorer shows for it; found
/// once per process.
fn default_name() -> Option<&'static Name> {
    static NAME: OnceLock<Option<Name>> = OnceLock::new();
    NAME.get_or_init(|| {
        let mut found: Option<Name> = None;
        // SAFETY: `first_name` writes only through `lparam`, which points to
        // `found` for the duration of the call; the enumeration stops (and
        // reports a failure, ignored here) once it returns FALSE.
        let _ = unsafe {
            EnumResourceNamesW(
                None,
                RT_GROUP_ICON,
                Some(first_name),
                &mut found as *mut Option<Name> as isize,
            )
        };
        found
    })
    .as_ref()
}

unsafe extern "system" fn first_name(
    _module: HMODULE,
    _kind: PCWSTR,
    name: PCWSTR,
    lparam: isize,
) -> BOOL {
    let value = name.0 as usize;
    let name = if value >> 16 == 0 {
        // `IS_INTRESOURCE`: the high word of an integer id is zero.
        Name::Id(value as u16)
    } else {
        // SAFETY: a string name is a null-terminated string valid for the
        // duration of the callback.
        let mut text = unsafe { name.as_wide() }.to_vec();
        text.push(0);
        Name::Text(text)
    };
    // SAFETY: `lparam` is the `Option<Name>` `default_name` passed.
    unsafe { *(lparam as *mut Option<Name>) = Some(name) };
    FALSE
}

/// The executable's default icon at the system's icon size, for a window
/// class; a null handle when the executable has none. Loaded shared, so the
/// system owns it and repeated calls return the same handle. The class's small
/// icon is left null: the system then picks it from this icon's resource,
/// which a shared load of a second size is not guaranteed to do.
pub(super) fn class_icon() -> HICON {
    let Some(name) = default_name() else {
        return HICON::default();
    };
    // SAFETY: a plain metric query.
    let size = unsafe { GetSystemMetrics(SM_CXICON) };
    load_resource(name, size, LR_SHARED).unwrap_or_default()
}

/// Loads the icon resource `name` from the executable at `size` pixels.
fn load_resource(name: &Name, size: i32, flags: IMAGE_FLAGS) -> Option<HICON> {
    // SAFETY: a module-handle query, then a load from that module; `name`
    // outlives the call.
    unsafe {
        let module = GetModuleHandleW(None).ok()?;
        let handle = LoadImageW(
            Some(module.into()),
            name.as_pcwstr(),
            IMAGE_ICON,
            size,
            size,
            flags,
        )
        .ok()?;
        Some(HICON(handle.0))
    }
}

/// The big and small icon sizes at `dpi`.
fn sizes(dpi: u32) -> (i32, i32) {
    // SAFETY: plain metric queries.
    unsafe {
        (
            GetSystemMetricsForDpi(SM_CXICON, dpi),
            GetSystemMetricsForDpi(SM_CXSMICON, dpi),
        )
    }
}

/// `icon` at the sizes a window at `dpi` shows, or the executable's default
/// icon when `icon` is `None`. `None` when there is nothing to load.
pub(crate) fn load(icon: Option<&WindowIcon>, dpi: u32) -> Option<WindowIcons> {
    let (big, small) = sizes(dpi);
    let at = |size| match icon {
        Some(WindowIcon::Resource(id)) => owned_resource(&Name::Id(*id), size),
        Some(images) => from_images(images, size),
        None => owned_resource(default_name()?, size),
    };
    Some(WindowIcons {
        big: at(big)?,
        small: at(small)?,
    })
}

fn owned_resource(name: &Name, size: i32) -> Option<Icon> {
    load_resource(name, size, LR_DEFAULTCOLOR).map(Icon)
}

/// The image of `icon` best suited to `size`, resampled to exactly that size.
fn from_images(icon: &WindowIcon, size: i32) -> Option<Icon> {
    let side = u32::try_from(size).ok()?;
    let image = icon.closest(side)?.resized(side, side).ok()?;
    // `CreateIcon` takes top-down BGRA colour bits and a 1-bit AND mask whose
    // rows are padded to 16 bits. An all-zero mask leaves the alpha channel to
    // decide what shows.
    let mut bgra = image.into_pixels();
    for pixel in bgra.as_chunks_mut::<4>().0 {
        pixel.swap(0, 2);
    }
    let mask = vec![0u8; side.div_ceil(16) as usize * 2 * side as usize];
    // SAFETY: `mask` and `bgra` hold exactly the bits a `size` x `size` icon
    // with a 1-bit mask and 32-bit colour needs, and outlive the call.
    let handle = unsafe { CreateIcon(None, size, size, 1, 32, mask.as_ptr(), bgra.as_ptr()) };
    handle.ok().map(Icon)
}

/// Shows `icons` on the window. The caller keeps them alive while it does.
pub(crate) fn apply(hwnd: Hwnd, icons: &WindowIcons) {
    for (kind, icon) in [(ICON_BIG, &icons.big), (ICON_SMALL, &icons.small)] {
        // SAFETY: `WM_SETICON` with a live icon handle; the window does not
        // take ownership of it.
        unsafe {
            SendMessageW(
                hwnd.raw(),
                WM_SETICON,
                Some(WPARAM(kind as usize)),
                Some(LPARAM(icon.0.0 as isize)),
            )
        };
    }
}
