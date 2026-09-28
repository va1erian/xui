//! Shared plumbing for the one-file-per-widget demos in this directory.
//!
//! This lives in a subdirectory, so cargo does not discover it as an example
//! of its own; every `control_*.rs` includes it with
//! `#[path = "support.rs"] mod support;`.

use std::rc::Rc;

use xui_core::app::Ui;
use xui_core::backend::Backend;
use xui_core::{Dip, Rect};

use xui_canvas::WinitBackend;
#[cfg(all(feature = "d2d", windows))]
use xui_win32::Win32Backend;

/// The backend a demo runs on: the native Win32 backend where it exists unless
/// `XUI_BACKEND=canvas`, otherwise the portable software backend.
pub fn backend() -> Rc<dyn Backend> {
    #[cfg(all(feature = "d2d", windows))]
    {
        if std::env::var("XUI_BACKEND").as_deref() != Ok("canvas") {
            return Rc::new(Win32Backend::new());
        }
    }
    Rc::new(WinitBackend::new())
}

/// Converts design-unit coordinates to device pixels at one window's DPI.
#[derive(Clone, Copy)]
pub struct Layout {
    dpi: u32,
}

impl Layout {
    /// A converter for a window reporting `dpi` dots per inch.
    pub fn new(dpi: u32) -> Layout {
        Layout { dpi }
    }

    /// A design offset or size in device pixels.
    pub fn dip(&self, value: f32) -> i32 {
        Dip(value).to_px(self.dpi).value()
    }

    /// A design rectangle (`left`, `top`, `right`, `bottom`) in device pixels.
    pub fn rect(&self, left: f32, top: f32, right: f32, bottom: f32) -> Rect {
        Rect::new(
            self.dip(left),
            self.dip(top),
            self.dip(right),
            self.dip(bottom),
        )
    }
}

/// Makes a demo quit itself when `XUI_DEMO_AUTOCLOSE_MS` is set, raising
/// `quit` after that many milliseconds so every demo is smoke-testable.
pub fn autoclose<M: 'static>(ui: &Ui<M>, quit: impl Fn() -> M + 'static) {
    let Ok(millis) = std::env::var("XUI_DEMO_AUTOCLOSE_MS") else {
        return;
    };
    let Ok(millis) = millis.parse::<u32>() else {
        return;
    };
    let at = ui.set_timer(millis);
    ui.on_timer(move |fired| if fired == at { Some(quit()) } else { None });
}
