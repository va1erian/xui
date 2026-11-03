#![forbid(unsafe_code)]

//! The order of pointer, focus and capture events, on a real (hidden) window.
//! The window is never shown, so the tests take no focus; run them in the
//! sandbox like every test that creates a window.

use std::cell::RefCell;
use std::rc::Rc;

use windows::Win32::UI::WindowsAndMessaging::WM_CAPTURECHANGED;
use xui_core::Rect;
use xui_core::backend::{
    Backend, Event, NodeKind, NodeSpec, ParentRef, PlatformSpec, WidgetId, WindowId,
};
use xui_core::message::{Key, Modifiers, MouseButton};
use xui_core::router::WidgetHost;

use super::D2dBackend;
use crate::sys::window::Msg;

/// Records `(target, event)` in delivery order.
#[derive(Default)]
struct Log(RefCell<Vec<(WidgetId, Event)>>);

impl WidgetHost for Log {
    fn deliver(&self, target: WidgetId, event: &Event) -> bool {
        self.0.borrow_mut().push((target, *event));
        true
    }
}

impl Log {
    fn take(&self) -> Vec<(WidgetId, &'static str)> {
        self.0
            .borrow_mut()
            .drain(..)
            .map(|(target, event)| (target, kind(&event)))
            .collect()
    }
}

fn kind(event: &Event) -> &'static str {
    match event {
        Event::MouseDown { .. } => "down",
        Event::MouseUp { .. } => "up",
        Event::MouseMove { .. } => "move",
        Event::MouseLeave => "leave",
        Event::SetFocus => "set-focus",
        Event::KillFocus => "kill-focus",
        Event::CaptureChanged => "capture-changed",
        _ => "other",
    }
}

struct Fixture {
    backend: D2dBackend,
    window: WindowId,
    log: Rc<Log>,
    a: WidgetId,
    b: WidgetId,
}

/// Two nodes side by side: `a` at x 0..50 and `b` at x 60..110, both 50 high.
fn fixture() -> Fixture {
    let backend = D2dBackend::new();
    backend.init();
    let window = backend
        .open_window(&PlatformSpec::new("input test"))
        .unwrap();
    let log = Rc::new(Log::default());
    backend.set_event_sink(window, Rc::clone(&log) as Rc<dyn WidgetHost>);
    let node = |x: i32| {
        backend
            .create(
                ParentRef::Window(window),
                &NodeSpec::new(NodeKind::Custom, Rect::new(x, 0, x + 50, 50)),
            )
            .unwrap()
    };
    let (a, b) = (node(0), node(60));
    Fixture {
        backend,
        window,
        log,
        a,
        b,
    }
}

fn mv(x: i32) -> Event {
    Event::MouseMove {
        x,
        y: 10,
        modifiers: Modifiers::NONE,
    }
}

fn press(x: i32) -> Event {
    Event::MouseDown {
        x,
        y: 10,
        button: MouseButton::Left,
        modifiers: Modifiers::NONE,
    }
}

#[test]
fn moving_onto_another_node_first_tells_the_old_one_it_left() {
    let f = fixture();
    f.backend.shared.pointer(f.window, mv(10));
    f.log.take();
    f.backend.shared.pointer(f.window, mv(70));
    assert_eq!(f.log.take(), vec![(f.a, "leave"), (f.b, "move")]);
}

#[test]
fn a_press_focuses_its_node_before_the_node_sees_it() {
    let f = fixture();
    f.backend.shared.pointer(f.window, press(10));
    assert_eq!(f.log.take(), vec![(f.a, "set-focus"), (f.a, "down")]);
    f.backend.shared.pointer(f.window, press(70));
    assert_eq!(
        f.log.take(),
        vec![(f.a, "kill-focus"), (f.b, "set-focus"), (f.b, "down")]
    );
}

#[test]
fn a_second_press_on_the_focused_node_does_not_refocus_it() {
    let f = fixture();
    f.backend.shared.pointer(f.window, press(10));
    f.log.take();
    f.backend.shared.pointer(f.window, press(20));
    assert_eq!(f.log.take(), vec![(f.a, "down")]);
}

#[test]
fn a_press_on_bare_window_takes_the_focus_from_a_popup_only() {
    let f = fixture();
    let popup = f
        .backend
        .create(
            ParentRef::Window(f.window),
            &NodeSpec::new(NodeKind::Custom, Rect::new(200, 0, 250, 50)).popup(),
        )
        .unwrap();
    f.backend.focus(popup);
    f.log.take();
    f.backend.shared.pointer(f.window, press(400));
    assert_eq!(f.log.take(), vec![(popup, "kill-focus")]);
    assert_eq!(f.backend.focus_holder(f.window), None);

    // An ordinary node keeps the focus when the bare window is pressed.
    f.backend.focus(f.a);
    f.log.take();
    f.backend.shared.pointer(f.window, press(400));
    assert!(f.log.take().is_empty());
    assert_eq!(f.backend.focus_holder(f.window), Some(f.a));
}

#[test]
fn a_captured_node_gets_moves_outside_it_in_its_own_coordinates() {
    let f = fixture();
    f.backend.set_capture(f.a);
    f.backend.shared.pointer(f.window, mv(70));
    let events = f.log.0.borrow().clone();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].0, f.a);
    assert_eq!(events[0].1.position(), Some((70, 10)));
    f.backend.release_capture();
}

#[test]
fn releasing_the_capture_tells_the_node_once() {
    let f = fixture();
    f.backend.set_capture(f.a);
    f.log.take();
    f.backend.release_capture();
    assert_eq!(f.log.take(), vec![(f.a, "capture-changed")]);
    // The system's own notification for that release finds nothing to report.
    let hwnd = f.backend.shared.hwnd_of(f.window).unwrap();
    f.backend.shared.handle(
        f.window,
        hwnd,
        Msg {
            id: WM_CAPTURECHANGED,
            wparam: 0,
            lparam: 0,
        },
    );
    assert!(f.log.take().is_empty());
}

#[test]
fn capture_taken_by_windows_is_reported_and_stops_the_routing() {
    let f = fixture();
    f.backend.set_capture(f.a);
    f.log.take();
    let hwnd = f.backend.shared.hwnd_of(f.window).unwrap();
    f.backend.shared.handle(
        f.window,
        hwnd,
        Msg {
            id: WM_CAPTURECHANGED,
            wparam: 0,
            lparam: 0,
        },
    );
    assert_eq!(f.log.take(), vec![(f.a, "capture-changed")]);
    // Pointer events route by position again.
    f.backend.shared.pointer(f.window, mv(70));
    assert_eq!(f.log.take(), vec![(f.b, "move")]);
}

#[test]
fn tab_moves_the_focus_through_the_tab_stops_and_wraps() {
    let backend = D2dBackend::new();
    backend.init();
    let window = backend.open_window(&PlatformSpec::new("tab test")).unwrap();
    let log = Rc::new(Log::default());
    backend.set_event_sink(window, Rc::clone(&log) as Rc<dyn WidgetHost>);
    let stop = |x: i32| {
        backend
            .create(
                ParentRef::Window(window),
                &NodeSpec::new(NodeKind::Custom, Rect::new(x, 0, x + 50, 50)).tab_stop(),
            )
            .unwrap()
    };
    let (first, second) = (stop(0), stop(60));
    let tab = Msg {
        id: 0,
        wparam: usize::from(Key::TAB.code()),
        lparam: 0,
    };
    backend.shared.key_down(window, tab, false);
    assert_eq!(backend.focus_holder(window), Some(first));
    backend.shared.key_down(window, tab, false);
    assert_eq!(backend.focus_holder(window), Some(second));
    backend.shared.key_down(window, tab, false);
    assert_eq!(backend.focus_holder(window), Some(first));
}

#[test]
fn a_held_key_reports_a_repeat_and_a_fresh_press_does_not() {
    let msg = |lparam| Msg {
        id: 0,
        wparam: 0,
        lparam,
    };
    assert_eq!(super::keys::repeat_of(msg(1)), 1);
    assert_eq!(super::keys::repeat_of(msg(1 | (1 << 30))), 2);
}
