//! A portable "kitchen sink" demo for the `xui-canvas` backend: an
//! emusic-shaped shell (menu bar, transport top bar, navigator tree, four
//! central pages) over the complex portable widgets, so the canvas path can be
//! exercised end to end.
//!
//! Run the window on the software backend:
//!
//! ```text
//! cargo run -p xui-canvas --example kitchen_sink
//! ```
//!
//! Environment:
//! - `XUI_KITCHEN_THEME=dark` starts on the dark palette.
//! - `XUI_DEMO_AUTOCLOSE_MS=4000` makes the window quit itself (smoke runs).
//! - `XUI_KITCHEN_SNAPSHOT=<dir>` renders every page in light and dark to PNGs
//!   headlessly (no window), which is how the canvas path is inspected in CI.

mod app;
mod build;
mod data;

use std::rc::Rc;

use xui_canvas::{OffscreenBackend, RgbaImage, WinitBackend};
use xui_core::app::{App as _, Ui, run_app};
use xui_core::backend::{Backend, PlatformSpec};
use xui_core::{Dip, Theme};

use app::Msg;

fn spec() -> PlatformSpec {
    PlatformSpec::new("xui portable kitchen sink").size(Dip(build::WIDTH), Dip(build::HEIGHT))
}

fn main() {
    if let Ok(dir) = std::env::var("XUI_KITCHEN_SNAPSHOT") {
        snapshot(&dir);
        return;
    }
    let backend: Rc<dyn Backend> = Rc::new(WinitBackend::new());
    let _ = run_app(backend, spec(), |ui| {
        if std::env::var("XUI_KITCHEN_THEME").as_deref() == Ok("dark") {
            ui.set_theme(Theme::dark());
        }
        let app = build::build(ui);
        autoclose(ui);
        app
    });
}

/// Quits the window after `XUI_DEMO_AUTOCLOSE_MS`, so the demo is
/// smoke-testable without a person at the keyboard.
fn autoclose(ui: &Ui<Msg>) {
    let Ok(millis) = std::env::var("XUI_DEMO_AUTOCLOSE_MS") else {
        return;
    };
    let Ok(millis) = millis.parse::<u32>() else {
        return;
    };
    let at = ui.set_timer(millis);
    ui.on_timer(move |fired| (fired == at).then_some(Msg::Autoclose));
}

/// Renders each page in both themes through the offscreen backend and writes a
/// PNG per page/theme. No OS window is created.
fn snapshot(dir: &str) {
    let backend = Rc::new(OffscreenBackend::new());
    let run_backend: Rc<dyn Backend> = backend.clone();
    let make_backend = Rc::clone(&backend);
    let dir = dir.to_string();
    let _ = run_app(run_backend, spec(), move |ui| {
        let mut app = build::build(ui);
        for page in 0..4 {
            for (dark, label) in [(false, "light"), (true, "dark")] {
                app.update(Msg::Theme(if dark { 1 } else { 0 }), ui);
                app.update(Msg::Navigate(page), ui);
                if let Some(image) = make_backend.render(ui.window()) {
                    save(&dir, &format!("kitchen-{page}-{label}.png"), &image);
                }
            }
        }
        app
    });
}

fn save(dir: &str, name: &str, image: &RgbaImage) {
    let path = std::path::Path::new(dir).join(name);
    let _ = std::fs::create_dir_all(dir);
    let file = std::fs::File::create(&path).expect("create snapshot");
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), image.width, image.height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .expect("png header")
        .write_image_data(&image.pixels)
        .expect("png data");
    eprintln!("wrote {}", path.display());
}
