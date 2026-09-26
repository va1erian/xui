//! The window-level OpenGL path: a [`GlWidget`] drawn by a real GL context on a
//! `winit` window.
//!
//! `winit` requires its event loop on the process's main thread (a hard panic on
//! Windows), which the libtest harness cannot provide, so this target uses
//! `harness = false` and runs from `main`. The test opens a real window and
//! needs an OpenGL driver, so it is opt-in: set `XUI_CANVAS_GL_TEST=1` to run
//! it. It also skips when the session has no display at all, and when no
//! context can be created (where the widget deliberately falls back to
//! software, so there is nothing to assert). A watchdog timer quits the loop
//! instead of hanging.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use xui_canvas::glow::{self, HasContext};
use xui_canvas::{GlWidget, WinitBackend};
use xui_core::app::{App, Ui, run_app};
use xui_core::backend::{Backend, PlatformSpec};
use xui_core::{Canvas, Dip, Rect, Theme};

/// A widget that records which paint path ran and the driver's GL version.
struct Probe {
    gl_ran: Rc<Cell<bool>>,
    fallback_ran: Rc<Cell<bool>>,
    version: Rc<RefCell<Option<String>>>,
}

impl GlWidget for Probe {
    fn paint(&self, canvas: &mut dyn Canvas, bounds: Rect, theme: &Theme) {
        self.fallback_ran.set(true);
        canvas.clear(theme.background);
        canvas.fill_rect(bounds, theme.accent);
    }

    fn paint_gl(&self, gl: &glow::Context, _bounds: Rect, _theme: &Theme) {
        self.gl_ran.set(true);
        if self.version.borrow().is_none() {
            // SAFETY: `gl` is current because a surface frame is active.
            let version = unsafe { gl.get_parameter_string(glow::VERSION) };
            self.version.borrow_mut().replace(version);
        }
    }
}

struct Demo;

impl App for Demo {
    type Msg = ();
    fn update(&mut self, _msg: (), _ui: &mut Ui<()>) {}
}

/// Whether the host can plausibly host a `winit` window on the main thread.
fn display_missing() -> bool {
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        return std::env::var_os("DISPLAY").is_none()
            && std::env::var_os("WAYLAND_DISPLAY").is_none();
    }
    #[allow(unreachable_code)]
    false
}

fn main() {
    std::process::exit(run());
}

/// `0` passes or skips, `1` fails.
fn run() -> i32 {
    if std::env::var("XUI_CANVAS_GL_TEST").as_deref() != Ok("1") {
        eprintln!("skip: set XUI_CANVAS_GL_TEST=1 to run the GL integration test");
        return 0;
    }
    if display_missing() {
        eprintln!("skip: no display available for a window");
        return 0;
    }

    let probe = Probe {
        gl_ran: Rc::new(Cell::new(false)),
        fallback_ran: Rc::new(Cell::new(false)),
        version: Rc::new(RefCell::new(None)),
    };
    let gl_ran = Rc::clone(&probe.gl_ran);
    let fallback_ran = Rc::clone(&probe.fallback_ran);
    let version = Rc::clone(&probe.version);

    let backend = Rc::new(WinitBackend::new());
    let backend_for_make = Rc::clone(&backend);
    let backend_for_timer = Rc::clone(&backend);
    let dynamic: Rc<dyn Backend> = backend;

    let run = run_app(
        dynamic,
        PlatformSpec::new("xui-canvas gl test").size(Dip(320.0), Dip(240.0)),
        move |ui| {
            backend_for_make.set_gl_content(ui.window(), probe);

            // Quit after a couple of frames, and fail the loop if it hangs.
            let quit = ui.set_timer(400);
            let watchdog = ui.set_timer(5000);
            ui.on_timer(move |fired| {
                if fired == quit {
                    backend_for_timer.quit(0);
                } else if fired == watchdog {
                    backend_for_timer.quit(1);
                }
                None
            });
            Demo
        },
    );
    if let Err(error) = run {
        eprintln!("fail: the app loop returned an error: {error}");
        return 1;
    }

    if !gl_ran.get() {
        if fallback_ran.get() {
            eprintln!("skip: no OpenGL driver, the widget fell back to software");
            return 0;
        }
        eprintln!("fail: neither the GL frame nor the software fallback ran");
        return 1;
    }
    let version = version.borrow();
    if version.as_deref().is_none_or(str::is_empty) {
        eprintln!("fail: the widget did not receive a live glow context");
        return 1;
    }
    println!(
        "ok: GL_VERSION = {}",
        version.as_deref().unwrap_or_default()
    );
    0
}
