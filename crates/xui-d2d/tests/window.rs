#![cfg(windows)]
// `pixels` reads the screen through GDI to check what was really drawn; that is
// the one place this test needs `unsafe`, so it is confined to that module.

//! A real window on the backend: it paints its nodes, fires timers, runs a
//! modal window, and what it draws is clipped as promised.
//!
//! Every loop here ends on a timer, and a later watchdog timer fails the test
//! instead of letting it hang. Run inside the sandbox
//! (`scripts\sandbox\run.ps1`): the windows are real and shown.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use xui_core::Rect;
use xui_core::backend::{
    Backend, Canvas, Event, NodeKind, NodeSpec, ParentRef, PlatformSpec, TimerId, WidgetId,
    WindowId,
};
use xui_core::color::Color;
use xui_core::router::WidgetHost;
use xui_d2d::D2dBackend;

mod pixels {
    #![allow(unsafe_code)]

    use windows::Win32::Foundation::POINT;
    use windows::Win32::Graphics::Gdi::{GetDC, GetPixel, ReleaseDC};
    use windows::Win32::UI::WindowsAndMessaging::FindWindowW;
    use windows::core::{HSTRING, PCWSTR};

    /// The on-screen colour at each client point of the window titled `title`.
    pub fn read(title: &str, points: &[(i32, i32)]) -> Vec<(u8, u8, u8)> {
        use windows::Win32::Graphics::Gdi::ClientToScreen;
        // SAFETY: a lookup, then reads of the screen through its device
        // context, released before returning; the points are plain values.
        unsafe {
            let hwnd = FindWindowW(PCWSTR::null(), &HSTRING::from(title)).expect("the test window");
            let screen = GetDC(None);
            let colours = points
                .iter()
                .map(|&(x, y)| {
                    let mut at = POINT { x, y };
                    let _ = ClientToScreen(hwnd, &mut at);
                    let value = GetPixel(screen, at.x, at.y).0;
                    (
                        (value & 0xFF) as u8,
                        ((value >> 8) & 0xFF) as u8,
                        ((value >> 16) & 0xFF) as u8,
                    )
                })
                .collect();
            ReleaseDC(None, screen);
            colours
        }
    }
}

/// Quits on one timer, fails on a later one, and runs `at_quit` first.
struct Sink {
    backend: Rc<D2dBackend>,
    quit_at: Cell<Option<TimerId>>,
    watchdog: Cell<Option<TimerId>>,
    timers: RefCell<Vec<TimerId>>,
    timed_out: Cell<bool>,
    at_quit: Box<dyn Fn()>,
}

impl Sink {
    fn new(backend: &Rc<D2dBackend>, at_quit: impl Fn() + 'static) -> Rc<Sink> {
        Rc::new(Sink {
            backend: Rc::clone(backend),
            quit_at: Cell::new(None),
            watchdog: Cell::new(None),
            timers: RefCell::default(),
            timed_out: Cell::new(false),
            at_quit: Box::new(at_quit),
        })
    }

    fn arm(&self, window: WindowId, quit_ms: u32) {
        self.quit_at
            .set(Some(self.backend.set_timer(window, quit_ms)));
        self.watchdog
            .set(Some(self.backend.set_timer(window, 8000)));
    }
}

impl WidgetHost for Sink {
    fn deliver(&self, _target: WidgetId, event: &Event) -> bool {
        match event {
            Event::Timer { id } if Some(*id) == self.quit_at.get() => {
                (self.at_quit)();
                self.backend.quit(0);
            }
            Event::Timer { id } if Some(*id) == self.watchdog.get() => {
                self.timed_out.set(true);
                self.backend.quit(1);
            }
            Event::Timer { id } => self.timers.borrow_mut().push(*id),
            _ => {}
        }
        true
    }
}

fn node(backend: &D2dBackend, window: WindowId, bounds: Rect) -> WidgetId {
    backend
        .create(
            ParentRef::Window(window),
            &NodeSpec::new(NodeKind::Custom, bounds),
        )
        .unwrap()
}

#[test]
fn a_window_paints_its_nodes_and_runs_timers() {
    let backend = Rc::new(D2dBackend::new());
    backend.init();
    let window = backend
        .open_window(&PlatformSpec::new("xui-d2d test"))
        .unwrap();
    let painted = Rc::new(Cell::new(false));
    let seen = Rc::clone(&painted);
    backend.set_painter(
        node(&backend, window, Rect::new(10, 10, 110, 60)),
        Rc::new(move |canvas: &mut dyn Canvas| {
            let bounds = canvas.bounds();
            canvas.fill_rect(bounds, Color::rgb(200, 40, 40));
            seen.set(true);
        }),
    );
    let sink = Sink::new(&backend, || {});
    backend.set_event_sink(window, Rc::clone(&sink) as Rc<dyn WidgetHost>);
    sink.timers.borrow_mut().push(backend.set_timer(window, 20));
    sink.arm(window, 400);

    let code = backend.run_with(window, &mut || {});
    backend.close_window(window);

    assert!(
        !sink.timed_out.get(),
        "the loop did not quit on its own timer"
    );
    assert_eq!(code, 0);
    assert!(painted.get(), "the node's painter never ran");
    assert!(
        sink.timers.borrow().len() > 1,
        "the repeating timer never fired"
    );
}

const RED: (u8, u8, u8) = (200, 40, 40);
const BLUE: (u8, u8, u8) = (30, 60, 220);
const GREEN: (u8, u8, u8) = (0, 160, 0);

#[test]
fn drawing_is_clipped_to_the_nodes_reach_and_a_leaked_clip_does_not_spread() {
    let title = "xui-d2d pixel test";
    let backend = Rc::new(D2dBackend::new());
    backend.init();
    let window = backend.open_window(&PlatformSpec::new(title)).unwrap();
    let fill = |color: Color, rect: Rect, leak_clip: bool| {
        Rc::new(move |canvas: &mut dyn Canvas| {
            if leak_clip {
                // A painter that forgets its pop_clip.
                canvas.push_clip(Rect::new(0, 0, 5, 5));
            }
            canvas.fill_rect(rect, color);
        })
    };
    // Fills its bounds.
    backend.set_painter(
        node(&backend, window, Rect::new(10, 10, 110, 60)),
        fill(
            Color::rgb(RED.0, RED.1, RED.2),
            Rect::new(0, 0, 100, 50),
            false,
        ),
    );
    // Draws 30 px beyond its bounds, past the 8 px overdraw margin.
    backend.set_painter(
        node(&backend, window, Rect::new(200, 10, 250, 60)),
        fill(
            Color::rgb(BLUE.0, BLUE.1, BLUE.2),
            Rect::new(-30, -30, 80, 80),
            false,
        ),
    );
    // Leaves a 5x5 clip pushed.
    backend.set_painter(
        node(&backend, window, Rect::new(300, 10, 350, 60)),
        fill(
            Color::rgb(BLUE.0, BLUE.1, BLUE.2),
            Rect::new(0, 0, 50, 50),
            true,
        ),
    );
    // Painted after the leaky one; it must still fill all of its bounds.
    backend.set_painter(
        node(&backend, window, Rect::new(400, 10, 450, 60)),
        fill(
            Color::rgb(GREEN.0, GREEN.1, GREEN.2),
            Rect::new(0, 0, 50, 50),
            false,
        ),
    );

    let points = [
        (50, 30),  // inside the red node
        (50, 80),  // below it: the background
        (225, 30), // inside the overdrawing node
        (170, 30), // 30 px left of it: outside its reach
        (302, 12), // inside the leaked 5x5 clip
        (340, 50), // inside the leaky node but outside its clip
        (425, 30), // the node after the leak
    ];
    let seen = Rc::new(RefCell::new(Vec::new()));
    let record = Rc::clone(&seen);
    let sink = Sink::new(&backend, move || {
        *record.borrow_mut() = pixels::read(title, &points);
    });
    backend.set_event_sink(window, Rc::clone(&sink) as Rc<dyn WidgetHost>);
    sink.arm(window, 1200);
    backend.run_with(window, &mut || {});
    backend.close_window(window);

    assert!(
        !sink.timed_out.get(),
        "the loop did not quit on its own timer"
    );
    let seen = seen.borrow();
    assert_eq!(seen[0], RED, "the node's own fill");
    assert_ne!(seen[1], RED, "below the node is not red");
    assert_eq!(seen[2], BLUE, "inside the overdrawing node");
    assert_ne!(seen[3], BLUE, "overdraw beyond the reach is clipped");
    assert_eq!(seen[4], BLUE, "inside the leaked clip");
    assert_ne!(seen[5], BLUE, "outside the leaked clip");
    assert_eq!(
        seen[6], GREEN,
        "a leaked clip does not spread to the next node"
    );
}

#[test]
fn a_modal_window_runs_its_own_loop_until_it_closes() {
    let backend = Rc::new(D2dBackend::new());
    backend.init();
    let owner = backend.open_window(&PlatformSpec::new("owner")).unwrap();
    let modal = backend.open_window(&PlatformSpec::new("modal")).unwrap();
    let closer = Rc::new(Cell::new(None::<TimerId>));
    let watchdog = Rc::new(Cell::new(None::<TimerId>));
    let timed_out = Rc::new(Cell::new(false));
    let closed_by_timer = Rc::new(Cell::new(false));

    struct Closer {
        closed_by_timer: Rc<Cell<bool>>,
        backend: Rc<D2dBackend>,
        window: WindowId,
        closer: Rc<Cell<Option<TimerId>>>,
        watchdog: Rc<Cell<Option<TimerId>>>,
        timed_out: Rc<Cell<bool>>,
    }
    impl WidgetHost for Closer {
        fn deliver(&self, _target: WidgetId, event: &Event) -> bool {
            if let Event::Timer { id } = event {
                if Some(*id) == self.watchdog.get() {
                    self.timed_out.set(true);
                }
                if Some(*id) == self.closer.get() {
                    self.closed_by_timer.set(true);
                }
                if Some(*id) == self.closer.get() || Some(*id) == self.watchdog.get() {
                    self.backend.close_window(self.window);
                }
            }
            true
        }
    }
    backend.set_event_sink(
        modal,
        Rc::new(Closer {
            closed_by_timer: Rc::clone(&closed_by_timer),
            backend: Rc::clone(&backend),
            window: modal,
            closer: Rc::clone(&closer),
            watchdog: Rc::clone(&watchdog),
            timed_out: Rc::clone(&timed_out),
        }),
    );
    closer.set(Some(backend.set_timer(modal, 200)));
    watchdog.set(Some(backend.set_timer(modal, 6000)));

    backend.set_window_enabled(owner, false);
    let result = backend.run_modal(modal);
    backend.set_window_enabled(owner, true);
    backend.close_window(owner);

    assert!(result.is_ok());
    assert!(
        closed_by_timer.get(),
        "run_modal returned before the modal window closed itself"
    );
    assert!(
        !timed_out.get(),
        "the modal loop did not end when its window closed"
    );
}
