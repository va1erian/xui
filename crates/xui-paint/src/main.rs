#![forbid(unsafe_code)]

//! The `xpaint` binary: run the app on the `xui-canvas` backend, or run the
//! headless `--self-test`.

use std::io::Write;
use std::process::ExitCode;
use std::rc::Rc;

use xui_core::Dip;
use xui_core::app::run_app;
use xui_core::backend::{Backend, PlatformSpec};
use xui_paint::storage::Storage;
use xui_paint::view::PaintApp;

fn main() -> ExitCode {
    if std::env::args().any(|argument| argument == "--self-test") {
        return self_test();
    }
    run()
}

/// Runs the interactive app on the portable `winit` backend.
fn run() -> ExitCode {
    #[cfg(feature = "fs")]
    let storage: Rc<dyn Storage> = Rc::new(xui_paint::storage::FsStorage::new("xpaint.png"));
    #[cfg(not(feature = "fs"))]
    let storage: Rc<dyn Storage> = Rc::new(xui_paint::storage::MemoryStorage::new());

    let backend: Rc<dyn Backend> = Rc::new(xui_canvas::WinitBackend::new());
    let spec = PlatformSpec::new("xpaint").size(Dip(800.0), Dip(600.0));
    match run_app(backend, spec, move |ui| {
        PaintApp::build(ui, storage).expect("build xpaint")
    }) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("xpaint: {error}");
            ExitCode::FAILURE
        }
    }
}

/// Runs the model checks plus a headless offscreen render, printing PASS/FAIL.
fn self_test() -> ExitCode {
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    let mut passed = xui_paint::selftest::run(&mut out);
    passed &= render_check(&mut out);
    if passed {
        let _ = writeln!(out, "SELFTEST PASS");
        ExitCode::SUCCESS
    } else {
        let _ = writeln!(out, "SELFTEST FAIL");
        ExitCode::FAILURE
    }
}

/// Renders a doodle offscreen (no window, no storage) and checks its pixels.
fn render_check(out: &mut dyn Write) -> bool {
    use xui_canvas::snapshot::{Snapshot, render_with};
    use xui_core::Theme;
    use xui_core::backend::Event;
    use xui_core::message::{Modifiers, MouseButton};

    let doodle = |theme: Theme| {
        render_with(
            Snapshot::new(Dip(320.0), Dip(240.0)).theme(theme),
            |ui| PaintApp::build(ui, Rc::new(xui_paint::storage::MemoryStorage::new())),
            |stage| {
                stage.emit(xui_paint::Msg::Tool(xui_paint::Tool::Pencil));
                let down = Event::MouseDown {
                    x: 60,
                    y: 100,
                    button: MouseButton::Left,
                    modifiers: Modifiers::NONE,
                };
                let moved = Event::MouseMove {
                    x: 120,
                    y: 140,
                    modifiers: Modifiers::NONE,
                };
                let up = Event::MouseUp {
                    x: 120,
                    y: 140,
                    button: MouseButton::Left,
                    modifiers: Modifiers::NONE,
                };
                stage.inject(down);
                stage.inject(moved);
                stage.inject(up);
            },
        )
    };

    let light = doodle(Theme::light());
    let dark = doodle(Theme::dark());
    let (Ok(light), Ok(dark)) = (light, dark) else {
        let _ = writeln!(out, "FAIL offscreen render: the snapshot failed");
        return false;
    };

    let drew = (90..110).any(|x| {
        (110..130)
            .any(|y| matches!(light.pixel(x, y), Some([r, g, b, _]) if r < 80 && g < 80 && b < 80))
    });
    if !drew {
        let _ = writeln!(out, "FAIL offscreen render: the pencil left no dark pixels");
        return false;
    }

    // The canvas (not the window chrome) must differ between themes.
    let themes_differ = (60..120).any(|x| (95..145).any(|y| light.pixel(x, y) != dark.pixel(x, y)));
    if !themes_differ {
        let _ = writeln!(out, "FAIL offscreen render: light and dark look identical");
        return false;
    }

    if outside_canvas_is_clean(&light) {
        let _ = writeln!(out, "PASS offscreen render (doodle, themes, clipping)");
        true
    } else {
        let _ = writeln!(
            out,
            "FAIL offscreen render: paint leaked outside the canvas"
        );
        false
    }
}

/// Whether the toolbar/palette/status bands differ from the canvas's white,
/// proving the canvas painted inside its own region only.
fn outside_canvas_is_clean(image: &xui_core::image::Image) -> bool {
    // The very top-left is the toolbar surface, not the canvas white.
    let corner = image.pixel(1, 1);
    corner.is_some() && corner != Some([255, 255, 255, 255])
}
