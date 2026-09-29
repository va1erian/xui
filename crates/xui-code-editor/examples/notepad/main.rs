#![forbid(unsafe_code)]

//! A small, cross-platform text editor: load, save, edit and search.
//!
//! It runs on the portable canvas backend ([`WinitBackend`]), so it works the
//! same on Windows, Linux and macOS. Paths are chosen by a command-line argument
//! or by typing one into a prompt dialog; xui has no native file picker yet.
//!
//! Run with:
//!
//! ```text
//! cargo run -p xui-code-editor --example notepad -- path/to/file.txt
//! ```
//!
//! `XUI_DEMO_AUTOCLOSE_MS` makes it quit itself for smoke runs; it skips the
//! discard confirmation. See the crate README for the shortcuts and the
//! no-native-dialog limitation.

mod app;
mod commands;
mod ui;

use std::path::PathBuf;
use std::rc::Rc;

use xui_canvas::WinitBackend;
use xui_core::app::{Ui, run_app};
use xui_core::backend::PlatformSpec;
use xui_core::units::Dip;

use app::Msg;

fn main() -> xui_core::backend::Result<()> {
    let path = std::env::args_os().nth(1).map(PathBuf::from);
    run_app(
        Rc::new(WinitBackend::new()),
        PlatformSpec::new("xui notepad").size(Dip(900.0), Dip(640.0)),
        move |ui| {
            let mut notepad = ui::build(ui).expect("the notepad's widgets built");
            if let Some(path) = path {
                commands::open_path(&mut notepad, ui, path);
            }
            autoclose(ui);
            notepad
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
    ui.on_timer(|_| Some(Msg::Autoclose));
}
