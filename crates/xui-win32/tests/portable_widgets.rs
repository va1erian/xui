//! Portable widgets (`xui_core::widget::{Label, Edit, Button}`) on the Win32
//! backend:
//! the app builds them, a worker thread clicks the button with real mouse
//! messages, and the button's painted pixels are sampled; the whole window is
//! also captured to a PNG for a visual check.
//!
//! Requires a real desktop; run it in Windows Sandbox like the rest of the UI
//! tests.

#![cfg(windows)]

mod common;

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use xui_core::app::{App, Ui, run_app};
use xui_core::backend::{Backend, PlatformSpec};
use xui_core::widget::{
    Button, CheckBox, ComboBox, Edit, GroupBox, HasText, Hyperlink, Label, ListView, ProgressBar,
    RadioGroup, Separator, Slider,
};
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
    text: Rc<RefCell<Option<String>>>,
    capture_at: Rc<Cell<Option<TimerId>>>,
    timed_out: Rc<Cell<bool>>,
    // Kept alive: each owns its node and destroys it on drop.
    _label: Label<Msg>,
    edit: Edit<Msg>,
    button: Button<Msg>,
    _check: CheckBox<Msg>,
    _bar: ProgressBar<Msg>,
    _slider: Slider<Msg>,
    _radios: RadioGroup<Msg>,
    _group: GroupBox<Msg>,
    _combo: ComboBox<Msg>,
    _list: ListView<Msg>,
    _link: Hyperlink<Msg>,
    _sep: Separator<Msg>,
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
                // Programmatic set_text must reach the native control and read
                // back consistently.
                self.edit.set_text("set by code");
                let shown = self.edit.text();
                let native = self.backend.text(self.edit.id());
                self.text.replace(Some(format!("{shown}|{native}")));
                if let Some(node) = self.backend.node_hwnd(self.button.id())
                    && let Some(rect) = common::screen_rect(node)
                    && let Some(image) = common::capture_screen(rect)
                    && !common::is_flat(&image)
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
    let text = Rc::new(RefCell::new(None));
    // After the click the button rests in its hover state.
    let expected = theme.hover;

    let result = {
        let timed_out = Rc::clone(&timed_out);
        let sample = Rc::clone(&sample);
        let text = Rc::clone(&text);
        let file = file.to_string();
        let backend = Rc::clone(&backend);
        run_app(
            backend_for_run,
            PlatformSpec::new("xui portable widgets"),
            move |ui| {
                ui.set_theme(theme);
                let label = Label::new(ui, Rect::new(20, 16, 320, 48), "Portable Label").unwrap();
                // A form editor's selection outline.
                label.set_selected(true);
                let edit = Edit::new(ui, Rect::new(20, 60, 320, 92), "")
                    .unwrap()
                    .on_change(|_| None);
                let edit_id = edit.id();
                edit.focus();
                let button = Button::new(ui, Rect::new(20, 108, 160, 144), "Click me")
                    .unwrap()
                    .on_click(|| Some(Msg::Clicked));
                let button_id: WidgetId = button.id();
                let check = CheckBox::new(ui, Rect::new(20, 156, 320, 184), "Enabled").unwrap();
                check.set_checked(true);
                let bar = ProgressBar::new(ui, Rect::new(20, 196, 320, 204), 100).unwrap();
                bar.set_value(60);
                let slider = Slider::new(ui, Rect::new(20, 216, 320, 244), 0.0, 100.0).unwrap();
                slider.set_value(40.0);
                let radios =
                    RadioGroup::new(ui, Rect::new(20, 252, 320, 336), &["Low", "Medium", "High"])
                        .unwrap();
                radios.select(1);
                let group = GroupBox::new(ui, Rect::new(340, 16, 620, 180), "Group").unwrap();
                let combo = ComboBox::new(
                    ui,
                    Rect::new(340, 200, 520, 228),
                    &["Alpha", "Beta", "Gamma"],
                )
                .unwrap();
                combo.select(1);
                let list = ListView::new(
                    ui,
                    Rect::new(340, 240, 620, 344),
                    &["Inbox", "Sent", "Drafts", "Archive"],
                )
                .unwrap();
                list.select(Some(1));
                let link = Hyperlink::new(ui, Rect::new(20, 352, 320, 380), "See docs").unwrap();
                let sep = Separator::new(ui, Rect::new(20, 392, 620, 394)).unwrap();
                let window = backend.window_hwnd(ui.window()).expect("window handle");

                // A worker types into the field and then clicks the button with
                // real messages, so the backend decodes them into portable
                // Events.
                let edit_node = backend.node_hwnd(edit_id);
                if let Some(node) = backend.node_hwnd(button_id) {
                    std::thread::spawn(move || {
                        // Click the field to focus it, then type, then click
                        // the button.
                        std::thread::sleep(std::time::Duration::from_millis(200));
                        if let Some(edit) = edit_node {
                            click(edit);
                        }
                        std::thread::sleep(std::time::Duration::from_millis(100));
                        if let Some(edit) = edit_node {
                            type_text(edit, "hello xui");
                        }
                        std::thread::sleep(std::time::Duration::from_millis(150));
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
                    text,
                    capture_at,
                    timed_out: Rc::clone(&timed_out),
                    _label: label,
                    edit,
                    button,
                    _check: check,
                    _bar: bar,
                    _slider: slider,
                    _radios: radios,
                    _group: group,
                    _combo: combo,
                    _list: list,
                    _link: link,
                    _sep: sep,
                }
            },
        )
    };

    if result.is_err() {
        return;
    }
    assert!(!timed_out.get(), "the watchdog fired before the click");
    // A non-rendering CI desktop yields no captured pixel; skip the visual
    // assertion rather than fail. The click and text sync are still checked.
    if let Some(pixel) = sample.get() {
        assert_eq!(
            pixel,
            [expected.r, expected.g, expected.b, 0xFF],
            "the button painted its hover colour"
        );
    }
    assert_eq!(
        text.borrow().as_deref(),
        Some("set by code|set by code"),
        "the edit text reached the native control and read back"
    );
}

/// Posts `text` to the node as `WM_CHAR` messages, as typing would.
fn type_text(node: xui_win32::Hwnd, text: &str) {
    use core::ffi::c_void;
    use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
    use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_CHAR};

    let hwnd = HWND(node.raw() as *mut c_void);
    for character in text.chars() {
        // SAFETY: `hwnd` is the live node window; the message carries a
        // character a real key press would.
        unsafe {
            let _ = PostMessageW(Some(hwnd), WM_CHAR, WPARAM(character as usize), LPARAM(0));
        }
    }
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
