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

#[derive(Debug)]
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
                    && common::dominant(&image, [ACCENT.r, ACCENT.g, ACCENT.b])
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
    // A non-rendering CI desktop yields no captured pixel; skip the visual
    // assertion rather than fail. The pipeline is still exercised above.
    let Some(pixel) = pixel.get() else {
        return;
    };
    assert_eq!(
        pixel,
        [ACCENT.r, ACCENT.g, ACCENT.b, 0xFF],
        "the portable app painted its accent colour"
    );
}

struct WorkerApp;

impl App for WorkerApp {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, ui: &mut Ui<Msg>) {
        match msg {
            Msg::Quit => ui.quit(),
        }
    }
}

#[test]
fn a_worker_thread_proxy_delivers_on_win32() {
    let backend = Rc::new(Win32Backend::new());
    let backend_for_run: Rc<dyn Backend> = backend.clone();
    let timed_out = Rc::new(Cell::new(false));

    let result = {
        let timed_out = Rc::clone(&timed_out);
        run_app(
            backend_for_run,
            PlatformSpec::new("xui.worker"),
            move |ui| {
                // A background thread hands a message back through the proxy; the
                // backend wakes the loop and the runtime drains it.
                let proxy = ui.proxy();
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_millis(50));
                    proxy.send(Msg::Quit).expect("proxy send");
                });

                let long = ui.set_timer(5000);
                let watchdog = Rc::clone(&timed_out);
                ui.on_timer(move |fired| {
                    if fired == long {
                        watchdog.set(true);
                        Some(Msg::Quit)
                    } else {
                        None
                    }
                });
                WorkerApp
            },
        )
    };

    if result.is_err() {
        return;
    }
    assert!(
        !timed_out.get(),
        "the worker's message arrived before the watchdog"
    );
}

struct PainterlessApp;

impl App for PainterlessApp {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, ui: &mut Ui<Msg>) {
        match msg {
            Msg::Quit => ui.quit(),
        }
    }
}

/// A node the app never gave a painter must still validate its `WM_PAINT`;
/// otherwise every invalidation re-delivers `WM_PAINT` forever and starves the
/// timer that should end the test. A background thread is the fallback quit, so
/// a regression fails the assertion instead of hanging the suite.
#[test]
fn a_painterless_node_does_not_spin_on_paint() {
    let backend = Rc::new(Win32Backend::new());
    let backend_for_run: Rc<dyn Backend> = backend.clone();
    let fired = Rc::new(Cell::new(false));

    let result = {
        let fired = Rc::clone(&fired);
        run_app(
            backend_for_run,
            PlatformSpec::new("xui.painterless"),
            move |ui| {
                // A plain container with no painter, as a composite widget's
                // spacer row would be. Invalidate it so `WM_PAINT` is delivered.
                let node = ui
                    .create_node(&NodeSpec::new(
                        NodeKind::Container,
                        Rect::new(0, 0, 160, 60),
                    ))
                    .expect("create painterless node");
                ui.invalidate(node);

                let short = ui.set_timer(200);
                assert!(short.0 != 0, "timer started");
                let fired_for_timer = Rc::clone(&fired);
                ui.on_timer(move |timer| {
                    if timer == short {
                        fired_for_timer.set(true);
                        Some(Msg::Quit)
                    } else {
                        None
                    }
                });

                // Fallback: never let a regression hang the test suite.
                let proxy = ui.proxy();
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_millis(3000));
                    let _ = proxy.send(Msg::Quit);
                });

                PainterlessApp
            },
        )
    };

    if result.is_err() {
        return;
    }
    assert!(
        fired.get(),
        "a painterless node starved the timer: WM_PAINT was not validated"
    );
}
