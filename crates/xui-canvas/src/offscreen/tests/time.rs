//! The offscreen backend has no clock unless a test scripts one, and counts
//! the frames the front layer asks for.

use std::time::Duration;

use super::*;

#[test]
fn there_is_no_clock_by_default() {
    assert_eq!(OffscreenBackend::new().now(), None);
}

#[test]
fn the_clock_follows_the_script() {
    let backend = OffscreenBackend::new();
    backend.set_now(Duration::from_millis(40));
    assert_eq!(backend.now(), Some(Duration::from_millis(40)));
    backend.advance(Duration::from_millis(16));
    assert_eq!(backend.now(), Some(Duration::from_millis(56)));
}

#[test]
fn advancing_an_unset_clock_starts_it_at_zero() {
    let backend = OffscreenBackend::new();
    backend.advance(Duration::from_millis(5));
    assert_eq!(backend.now(), Some(Duration::from_millis(5)));
}

#[test]
fn frame_requests_are_honoured_and_counted() {
    let backend = OffscreenBackend::new();
    let window = backend
        .open_window(&PlatformSpec::new("frames"))
        .expect("a window opens");
    assert_eq!(backend.frame_requests(), 0);
    assert!(backend.request_frame(window));
    assert!(backend.request_frame(window));
    assert_eq!(backend.frame_requests(), 2);
}

#[test]
fn an_injected_window_event_reaches_the_window_whatever_has_the_focus() {
    let backend = OffscreenBackend::new();
    let window = backend.open_window(&PlatformSpec::new("frames")).unwrap();
    let node = backend
        .create(
            ParentRef::Window(window),
            &NodeSpec::new(NodeKind::Edit, Rect::new(0, 0, 40, 20)).tab_stop(),
        )
        .unwrap();
    let log = Rc::new(RefCell::new(Vec::new()));
    backend.set_event_sink(window, Rc::new(Recorder(Rc::clone(&log))));
    backend.focus(node);
    log.borrow_mut().clear();

    backend.inject(window, Event::Frame);
    backend.inject(window, Event::Wake);
    backend.inject(window, Event::Char('x'));

    assert_eq!(
        *log.borrow(),
        vec![
            (WidgetId::NONE, Event::Frame),
            (WidgetId::NONE, Event::Wake),
            (node, Event::Char('x')),
        ],
        "window events go to the window, input to the focused node"
    );
}
