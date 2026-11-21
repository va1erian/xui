#![cfg(windows)]

//! `Backend::request_frame` on a real window: one `Event::Frame` per request,
//! paced to the display, and none once nobody asks.
//!
//! The loop ends on a timer, and a later watchdog timer fails the test instead
//! of letting it hang. Run inside the sandbox (`scripts\sandbox\run.ps1`): the
//! window is real and shown.

use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

use xui_core::backend::{Backend, Event, PlatformSpec, TimerId, WidgetId, WindowId};
use xui_core::router::WidgetHost;
use xui_d2d::D2dBackend;

/// How many frames the sink asks for before it stops asking.
const WANTED: u32 = 20;

/// Asks for [`WANTED`] frames one after another, then for none.
struct Sink {
    backend: Rc<D2dBackend>,
    window: WindowId,
    frames: Cell<u32>,
    /// The backend's clock at the first and the last frame.
    span: Cell<(Duration, Duration)>,
    quit_at: Cell<Option<TimerId>>,
    watchdog: Cell<Option<TimerId>>,
    timed_out: Cell<bool>,
}

impl WidgetHost for Sink {
    fn deliver(&self, target: WidgetId, event: &Event) -> bool {
        match event {
            Event::Frame => {
                assert!(target.is_none(), "a frame goes to the window");
                let now = self.backend.now().expect("the backend has a clock");
                let first = if self.frames.get() == 0 {
                    now
                } else {
                    self.span.get().0
                };
                self.span.set((first, now));
                self.frames.set(self.frames.get() + 1);
                if self.frames.get() < WANTED {
                    assert!(self.backend.request_frame(self.window));
                }
            }
            Event::Timer { id } if Some(*id) == self.quit_at.get() => self.backend.quit(0),
            Event::Timer { id } if Some(*id) == self.watchdog.get() => {
                self.timed_out.set(true);
                self.backend.quit(1);
            }
            _ => {}
        }
        true
    }
}

#[test]
fn a_frame_is_delivered_per_request_at_the_display_rate_and_then_none() {
    let backend = Rc::new(D2dBackend::new());
    backend.init();
    let window = backend
        .open_window(&PlatformSpec::new("xui-d2d frames"))
        .unwrap();
    let sink = Rc::new(Sink {
        backend: Rc::clone(&backend),
        window,
        frames: Cell::new(0),
        span: Cell::new((Duration::ZERO, Duration::ZERO)),
        quit_at: Cell::new(None),
        watchdog: Cell::new(None),
        timed_out: Cell::new(false),
    });
    backend.set_event_sink(window, Rc::clone(&sink) as Rc<dyn WidgetHost>);

    let before = backend.now().expect("the backend has a clock");
    assert!(backend.request_frame(window));
    // Asking twice before the frame arrives still delivers one.
    assert!(backend.request_frame(window));
    // Long enough for the frames at 30 Hz, then as long again with none asked.
    sink.quit_at.set(Some(backend.set_timer(window, 1500)));
    sink.watchdog.set(Some(backend.set_timer(window, 8000)));

    let code = backend.run_with(window, &mut || {});
    backend.close_window(window);

    assert!(!sink.timed_out.get(), "the loop did not quit on its timer");
    assert_eq!(code, 0);
    assert_eq!(
        sink.frames.get(),
        WANTED,
        "one frame per request, and none after the last request"
    );
    let (first, last) = sink.span.get();
    assert!(first >= before, "the clock does not go backwards");
    let per_frame = (last - first) / (WANTED - 1);
    println!("frame interval: {per_frame:?}");
    // `SetTimer` could not do better than about 15.6 ms; a paced frame is at
    // most one 30 Hz refresh, with slack for a loaded sandbox.
    assert!(
        per_frame < Duration::from_millis(40),
        "frames {per_frame:?} apart are not paced to a display"
    );
    assert!(
        per_frame > Duration::from_millis(1),
        "frames {per_frame:?} apart are not paced at all"
    );
}
