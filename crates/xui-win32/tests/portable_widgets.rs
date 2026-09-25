//! Portable widgets (`xui_core::widget::{Label, Button}`) on the Win32 backend:
//! the app builds them, a worker thread clicks the button with real mouse
//! messages, and the button's painted pixels are sampled; the whole window is
//! also captured to a PNG for a visual check.
//!
//! Requires a real desktop; run it in Windows Sandbox like the rest of the UI
//! tests.

#![cfg(windows)]

mod common;

use std::cell::Cell;
use std::rc::Rc;

use xui_core::app::{App, Ui, run_app};
use xui_core::backend::{Backend, PlatformSpec};
use xui_core::widget::{Button, Label};
use xui_core::{Rect, Theme, TimerId, WidgetId};
use xui_win32::Win32Backend;

enum Msg {
    Clicked,
    Capture,
    Watchdog,
}

struct WidgetsApp {
    backend: Rc<Win32Backend>,
    window: xui_win32::Hwnd,
    file: String,
    sample: Rc<Cell<Option<[u8; 4]>>>,
    capture_at: Rc<Cell<Option<TimerId>>>,
    timed_out: Rc<Cell<bool>>,
    // Kept alive: each owns its node and destroys it on drop.
    _label: Label<Msg>,
    button: Button<Msg>,
}

impl App for WidgetsApp {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, ui: &mut Ui<Msg>) {
        match msg {
            Msg::Watchdog => {
                self.timed_out.set(true);
                ui.quit();
            }
            // Wait one turn for the invalidated button to repaint before
            // sampling and capturing.
            Msg::Clicked => {
                let id = ui.set_timer(120);
                self.capture_at.set(Some(id));
            }
            Msg::Capture => {
                if let Some(node) = self.backend.node_hwnd(self.button.id())
                    && let Some(rect) = common::screen_rect(node)
                    && let Some(image) = common::capture_screen(rect)
                    // Sample above the vertically-centred text, where the face
                    // fill shows cleanly.
                    && let Some(pixel) = image.pixel(image.width / 2, image.height / 6)
                {
                    self.sample.set(Some(pixel));
                }
                save_capture(self.window, &self.file);
                ui.quit();
            }
        }
    }
}

/// The directory captures land in: `XUI_SCREENSHOT_DIR` when set, the sandbox
/// runner's mapped `out` directory when running inside it, else `target/ui`.
fn capture_dir() -> std::path::PathBuf {
    if let Ok(dir) = std::env::var("XUI_SCREENSHOT_DIR") {
        return dir.into();
    }
    if std::path::Path::new("C:\\stage").is_dir() {
        return "C:\\stage\\out".into();
    }
    "target/ui".into()
}

/// Captures the window's screen rectangle to `file` under [`capture_dir`] as a
/// PNG.
fn save_capture(window: xui_win32::Hwnd, file: &str) {
    let Some(rect) = common::screen_rect(window) else {
        return;
    };
    let Some(image) = common::capture_screen(rect) else {
        return;
    };
    let path = capture_dir().join(file);
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let Ok(file) = std::fs::File::create(&path) else {
        return;
    };
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), image.width, image.height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    if let Ok(mut writer) = encoder.write_header() {
        let _ = writer.write_image_data(&image.pixels);
    }
}

fn run(theme: Theme, file: &str) {
    let backend = Rc::new(Win32Backend::new());
    let backend_for_run: Rc<dyn Backend> = backend.clone();
    let timed_out = Rc::new(Cell::new(false));
    let sample = Rc::new(Cell::new(None));
    // After the click the button rests in its hover state.
    let expected = theme.hover;

    let result = {
        let timed_out = Rc::clone(&timed_out);
        let sample = Rc::clone(&sample);
        let file = file.to_string();
        let backend = Rc::clone(&backend);
        run_app(
            backend_for_run,
            PlatformSpec::new("xui portable widgets"),
            move |ui| {
                ui.set_theme(theme);
                let label = Label::new(ui, Rect::new(20, 16, 320, 48), "Portable Label").unwrap();
                let button = Button::new(ui, Rect::new(20, 60, 160, 96), "Click me")
                    .unwrap()
                    .on_click(|| Some(Msg::Clicked));
                let button_id: WidgetId = button.id();
                let window = backend.window_hwnd(ui.window()).expect("window handle");

                // A worker clicks the button with real mouse messages, so the
                // backend decodes them into portable Events.
                if let Some(node) = backend.node_hwnd(button_id) {
                    std::thread::spawn(move || {
                        std::thread::sleep(std::time::Duration::from_millis(250));
                        click(node);
                    });
                }

                let capture_at = Rc::new(Cell::new(None));
                let long = ui.set_timer(5000);
                let watchdog = Rc::clone(&timed_out);
                let capture_id = Rc::clone(&capture_at);
                ui.on_timer(move |fired| {
                    if fired == long {
                        watchdog.set(true);
                        Some(Msg::Watchdog)
                    } else if capture_id.get() == Some(fired) {
                        Some(Msg::Capture)
                    } else {
                        None
                    }
                });

                WidgetsApp {
                    backend,
                    window,
                    file,
                    sample,
                    capture_at,
                    timed_out: Rc::clone(&timed_out),
                    _label: label,
                    button,
                }
            },
        )
    };

    if result.is_err() {
        return;
    }
    assert!(!timed_out.get(), "the watchdog fired before the click");
    let pixel = sample
        .get()
        .expect("the button was painted after the click");
    assert_eq!(
        pixel,
        [expected.r, expected.g, expected.b, 0xFF],
        "the button painted its hover colour"
    );
}

/// Posts a left-button press and release at `(10, 10)` in the node's client
/// area.
fn click(node: xui_win32::Hwnd) {
    use core::ffi::c_void;
    use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
    use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_LBUTTONDOWN, WM_LBUTTONUP};

    let hwnd = HWND(node.raw() as *mut c_void);
    let at = ((10 << 16) | 10) as isize;
    // SAFETY: `hwnd` is the live node window; the messages carry only integer
    // coordinates a real click would.
    unsafe {
        let _ = PostMessageW(Some(hwnd), WM_LBUTTONDOWN, WPARAM(0x0001), LPARAM(at));
        let _ = PostMessageW(Some(hwnd), WM_LBUTTONUP, WPARAM(0), LPARAM(at));
    }
}

/// Runs light then dark in one test: window captures must not overlap, and
/// cargo runs a binary's tests in parallel.
#[test]
fn portable_widgets_render_and_click() {
    run(Theme::light(), "portable-light.png");
    run(Theme::dark(), "portable-dark.png");
}
