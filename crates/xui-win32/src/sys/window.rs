//! Window classes and creation.

use core::ffi::c_void;

use windows::Win32::Foundation::{HINSTANCE, HWND};
use windows::Win32::Graphics::Gdi::HBRUSH;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CS_DBLCLKS, CreateWindowExW, HCURSOR, HMENU, IDC_ARROW, LoadCursorW, RegisterClassExW,
    UnregisterClassW, WINDOW_EX_STYLE, WINDOW_STYLE, WNDCLASSEXW,
};
use windows::core::{HSTRING, PCWSTR};

use crate::error::{Error, Result};
use crate::geometry::Rect;
use crate::hwnd::Hwnd;
use crate::window::WindowHandler;

use super::{raw_hwnd, win32, win32_error};

pub(crate) use super::window_ops::*;

/// The module (`HINSTANCE`) of the running executable.
fn module_instance() -> Result<HINSTANCE> {
    // SAFETY: a null module name asks for the current process's module, which
    // always exists.
    let module = unsafe { GetModuleHandleW(None) }.map_err(win32_error)?;
    Ok(HINSTANCE(module.0))
}

/// The shared arrow cursor.
fn arrow_cursor() -> Result<HCURSOR> {
    // SAFETY: `IDC_ARROW` is a system resource constant and a null module name
    // selects the shared system cursor.
    unsafe { LoadCursorW(None, IDC_ARROW) }.map_err(win32_error)
}

/// Registers a window class whose instances share [`window_proc`].
pub(crate) fn register_class(name: &[u16], display: &str, background: HBRUSH) -> Result<()> {
    let class = WNDCLASSEXW {
        cbSize: size_of::<WNDCLASSEXW>() as u32,
        // `CS_DBLCLKS` makes the class receive `WM_*BUTTONDBLCLK` messages.
        style: CS_DBLCLKS,
        lpfnWndProc: Some(super::dispatch::window_proc),
        hInstance: module_instance()?,
        hCursor: arrow_cursor()?,
        hbrBackground: background,
        lpszClassName: PCWSTR(name.as_ptr()),
        ..Default::default()
    };
    // SAFETY: `class` is fully initialised; `lpszClassName` points at `name`,
    // which the caller keeps alive for as long as any window uses the class.
    let atom = unsafe { RegisterClassExW(&class) };
    if atom == 0 {
        return Err(Error::ClassRegistration {
            name: display.to_string(),
        });
    }
    Ok(())
}

/// Unregisters a class previously passed to [`register_class`].
pub(crate) fn unregister_class(name: &[u16]) {
    if let Ok(instance) = module_instance() {
        // SAFETY: `name` is the same nul-terminated string used to register the
        // class; unregistering a class with no live windows is allowed.
        unsafe {
            let _ = UnregisterClassW(PCWSTR(name.as_ptr()), Some(instance));
        }
    }
}

/// The geometry and style bits for [`create`].
pub(crate) struct CreateParams<'a> {
    pub class_name: &'a [u16],
    pub title: &'a str,
    pub style: u32,
    pub ex_style: u32,
    pub bounds: Rect,
    pub parent: Option<Hwnd>,
    pub menu: isize,
}

/// Creates a window of a class registered by [`register_class`], taking
/// ownership of `handler` for the window's lifetime.
pub(crate) fn create<H: WindowHandler + 'static>(
    params: CreateParams<'_>,
    handler: H,
) -> Result<HWND> {
    let boxed: Box<dyn WindowHandler> = Box::new(handler);
    // A `*mut Box<dyn Trait>` is a thin pointer, which is what `lpCreateParams`
    // can carry; the fat vtable pointer lives inside the inner `Box`.
    let raw = Box::into_raw(Box::new(boxed));
    let title = HSTRING::from(params.title);
    let instance = module_instance()?;

    // SAFETY: all pointers passed are valid for the call: the class name and
    // `title` outlive it, and `raw` is an owned allocation reclaimed either
    // below or in `window_proc`.
    let created = unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE(params.ex_style),
            PCWSTR(params.class_name.as_ptr()),
            &title,
            WINDOW_STYLE(params.style),
            params.bounds.left,
            params.bounds.top,
            params.bounds.width(),
            params.bounds.height(),
            params.parent.map(raw_hwnd),
            Some(HMENU(params.menu as *mut c_void)),
            Some(instance),
            Some(raw as *const c_void),
        )
    };

    match created {
        Ok(hwnd) => Ok(hwnd),
        Err(source) => {
            // SAFETY: creation failed before WM_NCCREATE could adopt the
            // handler, so this is the only reference to it.
            unsafe { drop(Box::from_raw(raw)) };
            let end = params.class_name.len().saturating_sub(1);
            Err(Error::CreateWindow {
                class: String::from_utf16_lossy(&params.class_name[..end]),
                source: win32(source),
            })
        }
    }
}

/// Creates a child control from a system window class (e.g. `SysListView32`).
pub(crate) fn create_control(
    class: &str,
    style: u32,
    ex_style: u32,
    parent: Hwnd,
    id: usize,
    bounds: Rect,
) -> Result<HWND> {
    let class = HSTRING::from(class);
    let title = HSTRING::new();
    // SAFETY: `class` and `title` outlive the call; no create-params are used.
    unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE(ex_style),
            &class,
            &title,
            WINDOW_STYLE(style),
            bounds.left,
            bounds.top,
            bounds.width(),
            bounds.height(),
            Some(raw_hwnd(parent)),
            Some(HMENU(id as *mut c_void)),
            Some(module_instance()?),
            None,
        )
    }
    .map_err(win32_error)
}
