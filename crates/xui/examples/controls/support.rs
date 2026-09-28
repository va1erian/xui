//! Shared plumbing for the one-file-per-widget demos in this directory.
//!
//! This lives in a subdirectory, so cargo does not discover it as an example
//! of its own; every `control_*.rs` includes it with
//! `#[path = "support.rs"] mod support;`.

use std::cell::RefCell;
use std::rc::Rc;

use xui_core::app::Ui;
use xui_core::backend::Backend;
use xui_core::{Dip, Rect, Theme};

use xui_canvas::snapshot::Gallery;
use xui_canvas::{OffscreenBackend, WinitBackend};
#[cfg(all(feature = "d2d", windows))]
use xui_win32::Win32Backend;

thread_local! {
    /// The offscreen backend of a headless run, so [`snapshot_hook`] can
    /// schedule the capture on it once the demo is built.
    static OFFSCREEN: RefCell<Option<Rc<OffscreenBackend>>> = const { RefCell::new(None) };
}

/// The offscreen backend when `XUI_BACKEND=offscreen` or `XUI_SNAPSHOT` asks
/// for one, otherwise `None` and the demo runs as it always did.
pub fn offscreen_backend() -> Option<Rc<dyn Backend>> {
    if !Gallery::from_env().offscreen() {
        return None;
    }
    let backend = Rc::new(OffscreenBackend::new());
    OFFSCREEN.with(|slot| *slot.borrow_mut() = Some(Rc::clone(&backend)));
    Some(backend)
}

/// The name screenshots are saved under: the example's own name.
fn example_name() -> String {
    std::env::current_exe()
        .ok()
        .and_then(|exe| {
            exe.file_stem()
                .map(|stem| stem.to_string_lossy().into_owned())
        })
        .unwrap_or_else(|| "example".to_string())
}

/// In a headless run, arranges for the demo to save `<example>-light.png` and
/// `<example>-dark.png` into `XUI_SNAPSHOT` once it is built, and then to end
/// instead of waiting. Returns whether this is a headless run, so a caller can
/// skip its own timers.
pub fn snapshot_hook<M: 'static>(ui: &Ui<M>) -> bool {
    let Some(backend) = OFFSCREEN.with(|slot| slot.borrow().clone()) else {
        return false;
    };
    let gallery = Gallery::from_env();
    if gallery.dir().is_none() {
        return true;
    }
    let ui = ui.clone();
    let name = example_name();
    backend.set_run_hook(move || {
        for (variant, theme) in [("light", Theme::light()), ("dark", Theme::dark())] {
            ui.set_theme(theme);
            let saved = ui
                .capture()
                .map_err(xui_canvas::snapshot::SnapshotError::from)
                .and_then(|image| gallery.save(&name, variant, &image));
            if let Err(error) = saved {
                eprintln!("{name}-{variant}: {error}");
                std::process::exit(1);
            }
        }
    });
    true
}

/// The backend a demo runs on: the offscreen one when a headless run is asked
/// for, else the native Win32 backend where it exists unless
/// `XUI_BACKEND=canvas`, otherwise the portable software backend.
pub fn backend() -> Rc<dyn Backend> {
    if let Some(backend) = offscreen_backend() {
        return backend;
    }
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
/// `quit` after that many milliseconds so every demo is smoke-testable. In a
/// headless run it saves the screenshots instead (see [`snapshot_hook`]).
pub fn autoclose<M: 'static>(ui: &Ui<M>, quit: impl Fn() -> M + 'static) {
    if snapshot_hook(ui) {
        return;
    }
    let Ok(millis) = std::env::var("XUI_DEMO_AUTOCLOSE_MS") else {
        return;
    };
    let Ok(millis) = millis.parse::<u32>() else {
        return;
    };
    let at = ui.set_timer(millis);
    ui.on_timer(move |fired| if fired == at { Some(quit()) } else { None });
}
