#![cfg(windows)]

//! A real window on the backend: it paints its nodes, fires timers and wakes.
//! The loop ends on a timer, and a second, later timer fails the test instead
//! of letting it hang. Run inside the sandbox (`scripts\sandbox\run.ps1`).

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use xui_core::Rect;
use xui_core::backend::{
    Backend, Canvas, Event, NodeKind, NodeSpec, ParentRef, PlatformSpec, TimerId, WidgetId,
};
use xui_core::color::Color;
use xui_core::router::WidgetHost;
use xui_d2d::D2dBackend;

struct Sink {
    backend: Rc<D2dBackend>,
    window: xui_core::backend::WindowId,
    quit_at: Cell<Option<TimerId>>,
    watchdog: Cell<Option<TimerId>>,
    timers: RefCell<Vec<TimerId>>,
    timed_out: Cell<bool>,
}

impl WidgetHost for Sink {
    fn deliver(&self, _target: WidgetId, event: &Event) -> bool {
        match event {
            Event::Timer { id } if Some(*id) == self.quit_at.get() => self.backend.quit(0),
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

#[test]
fn a_window_paints_its_nodes_and_runs_timers() {
    let backend = Rc::new(D2dBackend::new());
    backend.init();
    let window = backend
        .open_window(&PlatformSpec::new("xui-d2d test"))
        .unwrap();
    let node = backend
        .create(
            ParentRef::Window(window),
            &NodeSpec::new(NodeKind::Custom, Rect::new(10, 10, 110, 60)),
        )
        .unwrap();
    let painted = Rc::new(Cell::new(false));
    let seen = Rc::clone(&painted);
    backend.set_painter(
        node,
        Rc::new(move |canvas: &mut dyn Canvas| {
            let bounds = canvas.bounds();
            canvas.fill_rect(bounds, Color::rgb(200, 40, 40));
            seen.set(true);
        }),
    );
    let sink = Rc::new(Sink {
        backend: Rc::clone(&backend),
        window,
        quit_at: Cell::new(None),
        watchdog: Cell::new(None),
        timers: RefCell::default(),
        timed_out: Cell::new(false),
    });
    backend.set_event_sink(window, Rc::clone(&sink) as Rc<dyn WidgetHost>);
    sink.timers.borrow_mut().push(backend.set_timer(window, 20));
    sink.quit_at.set(Some(backend.set_timer(window, 400)));
    sink.watchdog.set(Some(backend.set_timer(window, 8000)));

    let code = backend.run_with(window, &mut || {});
    backend.close_window(sink.window);

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
