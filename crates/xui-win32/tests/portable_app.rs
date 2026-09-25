//! The portable core runtime (`xui_core::app::run_app`) driven by the Win32
//! backend: the app builds a painted node, a timer ends it, and the node's
//! pixels are captured to prove the pipeline end to end.
//!
//! Requires a real desktop; run it in Windows Sandbox like the rest of the UI
//! tests.

#![cfg(windows)]

mod common;

use std::cell::Cell;
use std::rc::Rc;

use xui_core::app::{App, Ui, run_app};
use xui_core::backend::{Backend, NodeKind, NodeSpec, PlatformSpec};
use xui_core::{Color, Rect, WidgetId};
use xui_win32::Win32Backend;

/// The colour the node paints, chosen to be unmistakable in a capture.
const ACCENT: Color = Color::rgb(0x00, 0x78, 0xD4);

enum Msg {
    Quit,
}

struct PortApp {
    backend: Rc<Win32Backend>,
    node: WidgetId,
    pixel: Rc<Cell<Option<[u8; 4]>>>,
}

impl App for PortApp {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, ui: &mut Ui<Msg>) {
        match msg {
            Msg::Quit => {
                if let Some(hwnd) = self.backend.node_hwnd(self.node)
                    && let Some(rect) = common::screen_rect(hwnd)
                    && let Some(image) = common::capture_screen(rect)
                    && let Some(pixel) = image.pixel(image.width / 2, image.height / 2)
                {
                    self.pixel.set(Some(pixel));
                }
                ui.quit();
            }
        }
    }
}

#[test]
fn a_portable_app_runs_and_paints_on_the_win32_backend() {
    let backend = Rc::new(Win32Backend::new());
    let backend_for_run: Rc<dyn Backend> = backend.clone();

    let timed_out = Rc::new(Cell::new(false));
    let pixel = Rc::new(Cell::new(None));

    let result = {
        let timed_out = Rc::clone(&timed_out);
        let pixel = Rc::clone(&pixel);
        let backend = Rc::clone(&backend);
        run_app(
            backend_for_run,
            PlatformSpec::new("xui.portable"),
            move |ui| {
                let node = ui
                    .create_node(&NodeSpec::new(NodeKind::Custom, Rect::new(0, 0, 160, 80)))
                    .expect("create node");
                ui.set_painter(node, Rc::new(|canvas| canvas.clear(ACCENT)));

                // A short timer ends the test; a long one is the watchdog.
                let short = ui.set_timer(300);
                let long = ui.set_timer(5000);
                assert!(short.0 != 0 && long.0 != 0, "timers started");
                let watchdog = Rc::clone(&timed_out);
                ui.on_timer(move |fired| {
                    if fired == short {
                        Some(Msg::Quit)
                    } else if fired == long {
                        watchdog.set(true);
                        Some(Msg::Quit)
                    } else {
                        None
                    }
                });

                PortApp {
                    backend,
                    node,
                    pixel: Rc::clone(&pixel),
                }
            },
        )
    };

    // The window could not be created (no desktop): skip rather than fail.
    if result.is_err() {
        return;
    }
    assert!(!timed_out.get(), "the watchdog fired");
    let pixel = pixel.get().expect("a captured pixel");
    assert_eq!(
        pixel,
        [ACCENT.r, ACCENT.g, ACCENT.b, 0xFF],
        "the portable app painted its accent colour"
    );
}
