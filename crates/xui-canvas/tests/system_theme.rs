//! A real `winit` window opens in Daylight or Nightfall: the theme
//! `WinitBackend::system_theme` reads from the live window is applied before
//! the app is built. Which of the two the platform's preference maps to is
//! `theme_for`'s unit test.
//!
//! `winit` requires its event loop on the process's main thread, which the
//! libtest harness cannot provide, so this target uses `harness = false` and
//! runs from `main`. It opens a window, so on a shared desktop run it through
//! `scripts/sandbox/run.ps1`. It skips when the session has no display. A
//! watchdog thread outside the event loop ends the process if the loop never
//! gets as far as quitting, so a stalled loop fails instead of hanging.

use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

use xui_canvas::WinitBackend;
use xui_core::app::{App, Ui, run_app};
use xui_core::backend::{Backend, PlatformSpec};
use xui_core::{Dip, Theme};

struct Probe;

impl App for Probe {
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
    if display_missing() {
        eprintln!("skip: no display available for a window");
        return 0;
    }

    std::thread::spawn(|| {
        std::thread::sleep(Duration::from_secs(20));
        eprintln!("fail: the event loop did not quit within 20 s");
        std::process::exit(1);
    });

    let opened_in: Rc<Cell<Option<Theme>>> = Rc::new(Cell::new(None));
    let opened_in_for_make = Rc::clone(&opened_in);
    let quit_fired = Rc::new(Cell::new(false));
    let quit_fired_for_timer = Rc::clone(&quit_fired);
    let backend = Rc::new(WinitBackend::new());
    let backend_for_timer = Rc::clone(&backend);
    let dynamic: Rc<dyn Backend> = backend;

    let run = run_app(
        dynamic,
        PlatformSpec::new("xui-canvas system theme test").size(Dip(200.0), Dip(120.0)),
        move |ui| {
            opened_in_for_make.set(Some(ui.theme()));
            let quit = ui.set_timer(100);
            ui.on_timer(move |fired| {
                if fired == quit {
                    quit_fired_for_timer.set(true);
                    backend_for_timer.quit(0);
                }
                None
            });
            Probe
        },
    );
    if let Err(error) = run {
        eprintln!("fail: the app loop returned an error: {error}");
        return 1;
    }
    if !quit_fired.get() {
        eprintln!("fail: the event loop ended before the quit timer fired");
        return 1;
    }

    match opened_in.get() {
        Some(theme) if theme == Theme::daylight() || theme == Theme::nightfall() => {
            println!("ok: the window opened in the dark={} theme", theme.is_dark);
            0
        }
        Some(theme) => {
            eprintln!("fail: the window opened in {theme:?}, not Daylight or Nightfall");
            1
        }
        None => {
            eprintln!("fail: the app was never built");
            1
        }
    }
}
