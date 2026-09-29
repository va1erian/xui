#![forbid(unsafe_code)]

//! A spatial file explorer over the portable canvas backend and the std
//! filesystem: one window per folder, double-click to open, Delete to delete.
//!
//! ```text
//! cargo run -p xui-explorer --example explorer -- path/to/folder
//! ```
//!
//! With no argument it opens the home directory (or the current directory).
//! `XUI_DEMO_AUTOCLOSE_MS` makes it quit itself for a smoke run.

use std::path::PathBuf;
use std::rc::Rc;

use xui_canvas::WinitBackend;
use xui_core::app::{Ui, run_app};
use xui_core::backend::PlatformSpec;
use xui_core::units::Dip;

use xui_explorer::Explorer;
use xui_explorer::platform::Platform;
use xui_explorer::std_platform::{DesktopLauncher, StdPlatform};
use xui_explorer::window::Msg;

fn main() -> xui_core::backend::Result<()> {
    let platform = StdPlatform::new();
    let start = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .or_else(|| platform.home())
        .unwrap_or_else(|| PathBuf::from("."));

    let explorer = Explorer::new(Rc::new(platform), Rc::new(DesktopLauncher::new()));
    run_app(
        Rc::new(WinitBackend::new()),
        PlatformSpec::new("Explorer").size(Dip(720.0), Dip(480.0)),
        move |ui| {
            autoclose(ui);
            explorer.open_root(ui, start)
        },
    )
}

/// Quits after `XUI_DEMO_AUTOCLOSE_MS`, for a headless smoke run.
fn autoclose(ui: &mut Ui<Msg>) {
    let Ok(millis) = std::env::var("XUI_DEMO_AUTOCLOSE_MS") else {
        return;
    };
    let Ok(millis) = millis.parse::<u32>() else {
        return;
    };
    let _ = ui.set_timer(millis);
    let quit_ui = ui.clone();
    ui.on_timer(move |_| {
        quit_ui.quit();
        None
    });
}
