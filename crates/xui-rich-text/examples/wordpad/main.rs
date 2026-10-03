#![forbid(unsafe_code)]

//! A small word processor on the portable canvas backend: block kinds, font
//! sizes, bold/italic/underline/strike, alignment, lists, indentation, images
//! with text wrap, tables, undo/redo, JSON save/open and Markdown export.
//!
//! ```text
//! cargo run -p xui-rich-text --example wordpad
//! ```
//!
//! `WIN32UI_DEMO_AUTOCLOSE_MS` (or `XUI_DEMO_AUTOCLOSE_MS`) makes it quit
//! itself after that many milliseconds, for smoke runs.

mod app;
mod commands;
mod files;
mod table;
mod ui;

use std::rc::Rc;

use xui_canvas::WinitBackend;
use xui_core::app::{Ui, run_app};
use xui_core::backend::PlatformSpec;
use xui_core::units::Dip;

use app::Msg;

fn main() -> xui_core::backend::Result<()> {
    run_app(
        Rc::new(WinitBackend::new()),
        PlatformSpec::new("Untitled - Wordpad").size(Dip(1100.0), Dip(720.0)),
        |ui| {
            let wordpad = ui::build(ui).expect("the wordpad's widgets built");
            autoclose(ui);
            wordpad
        },
    )
}

/// Quits after the autoclose delay, if one is set.
fn autoclose(ui: &mut Ui<Msg>) {
    let millis = ["WIN32UI_DEMO_AUTOCLOSE_MS", "XUI_DEMO_AUTOCLOSE_MS"]
        .iter()
        .find_map(|name| std::env::var(name).ok())
        .and_then(|value| value.parse::<u32>().ok());
    if let Some(millis) = millis {
        let _ = ui.set_timer(millis);
        ui.on_timer(|_| Some(Msg::Autoclose));
    }
}
