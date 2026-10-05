//! Measures the paint latency of a portable `ListView` on the Win32 backend:
//! it repaints a 5000-row, four-column list many times with a synchronous
//! `WM_PAINT` and prints the milliseconds per paint.
//!
//! Ignored by default because it takes a few seconds and prints numbers rather
//! than asserting them:
//!
//! ```text
//! cargo test --release --test listview_paint_bench -- --ignored --nocapture
//! ```
//!
//! It is the measurement behind the Direct2D batching fix (#100): before the
//! fix every cell text and separator opened its own `BeginDraw`/`EndDraw`; now
//! one frame covers the whole paint.

#![cfg(windows)]

use std::cell::Cell;
use std::rc::Rc;
use std::time::Instant;

use xui_core::app::{App, Ui};
use xui_core::arrange::{Handle, LayoutExt, absolute, list};
use xui_core::backend::{Backend, PlatformSpec};
use xui_core::widget::{Fill, ListView};
use xui_core::{Dip, Theme};
use xui_win32::Win32Backend;

/// Rows in the model: enough that only a slice is visible and the rest is
/// virtualized away.
const ROWS: usize = 5_000;
/// Repaints measured for the per-paint figure.
const PAINTS: u64 = 100;

enum Msg {
    Bench,
}

struct Bench {
    backend: Rc<Win32Backend>,
    list: Handle<ListView<Msg>>,
    per_paint_ms: Rc<Cell<f64>>,
}

impl App for Bench {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, ui: &mut Ui<Msg>) {
        let Msg::Bench = msg;
        let Some(hwnd) = self.backend.node_hwnd(self.list.get().id()) else {
            ui.quit();
            return;
        };
        let start = Instant::now();
        for _ in 0..PAINTS {
            self.backend.invalidate(self.list.get().id());
            // SAFETY: a synchronous paint of the list's own live node window on
            // the UI thread; the handler is re-entrancy-safe.
            unsafe {
                use core::ffi::c_void;
                use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
                use windows::Win32::UI::WindowsAndMessaging::{SendMessageW, WM_PAINT};
                SendMessageW(
                    HWND(hwnd.raw() as *mut c_void),
                    WM_PAINT,
                    Some(WPARAM(0)),
                    Some(LPARAM(0)),
                );
            }
        }
        self.per_paint_ms
            .set(start.elapsed().as_secs_f64() * 1000.0 / PAINTS as f64);
        ui.quit();
    }
}

#[test]
#[ignore = "a multi-second measurement that prints numbers; run with --ignored --nocapture"]
fn listview_paint_latency() {
    let backend = Rc::new(Win32Backend::new());
    let backend_for_run: Rc<dyn Backend> = backend.clone();
    let per_paint_ms = Rc::new(Cell::new(f64::NAN));
    let spec = PlatformSpec::new("xui.listview.bench").size(Dip(720.0), Dip(460.0));

    let result = {
        let per_paint_ms = Rc::clone(&per_paint_ms);
        let backend = Rc::clone(&backend);
        xui_core::app::run_app(backend_for_run, spec, move |ui| {
            ui.set_theme(Theme::light());
            let model: Vec<Vec<String>> = (0..ROWS)
                .map(|index| {
                    vec![
                        format!("Track {}", index + 1),
                        format!("Artist {}", index % 37),
                        format!("Album {}", index % 13),
                        format!("{}", 1950 + index % 75),
                    ]
                })
                .collect();
            let handle = Handle::new();
            ui.root(
                absolute().child(
                    list()
                        .column("Title", Fill)
                        .column("Artist", Dip(140.0))
                        .column("Album", Dip(140.0))
                        .column_right("Year", Dip(56.0))
                        .then(|view: ListView<Msg>| {
                            view.set_model(model);
                            view
                        })
                        .bind(&handle)
                        .at(12, 12, 696, 388),
                ),
            )
            .unwrap();
            ui.emit(Msg::Bench);
            Bench {
                backend,
                list: handle,
                per_paint_ms,
            }
        })
    };

    if result.is_err() {
        return; // no desktop; skip
    }
    println!(
        "listview {} rows, {} columns: {:.3} ms per paint ({} paints)",
        ROWS,
        4,
        per_paint_ms.get(),
        PAINTS
    );
}
