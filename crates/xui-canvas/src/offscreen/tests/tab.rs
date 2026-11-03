//! Tab and Shift+Tab move the focus through the tab stops.

use super::*;

fn tab(shift: bool) -> Event {
    Event::KeyDown {
        key: xui_core::message::Key::TAB,
        modifiers: Modifiers {
            shift,
            ..Modifiers::NONE
        },
        repeat: 1,
        system: false,
    }
}

fn node(
    backend: &OffscreenBackend,
    window: xui_core::backend::WindowId,
    spec: NodeSpec,
) -> WidgetId {
    backend.create(ParentRef::Window(window), &spec).unwrap()
}

thread_local! {
    /// The next free row, so each node of a test sits apart from the others and
    /// none covers another.
    static ROW: std::cell::Cell<i32> = const { std::cell::Cell::new(0) };
}

fn spec() -> NodeSpec {
    let row = ROW.with(|row| row.replace(row.get() + 1));
    NodeSpec::new(NodeKind::Edit, Rect::new(0, row * 30, 40, row * 30 + 20))
}

#[test]
fn tab_walks_the_tab_stops_and_wraps() {
    let backend = OffscreenBackend::new();
    let window = backend.open_window(&PlatformSpec::new("tab")).unwrap();
    let first = node(&backend, window, spec().tab_stop());
    let _label = node(&backend, window, spec());
    let hidden = node(&backend, window, spec().tab_stop());
    backend.set_visible(hidden, false);
    let last = node(&backend, window, spec().tab_stop());
    let log = Rc::new(RefCell::new(Vec::new()));
    backend.set_event_sink(window, Rc::new(Recorder(Rc::clone(&log))));

    assert!(backend.inject(window, tab(false)));
    assert_eq!(
        backend.focus_holder(window),
        Some(first),
        "Tab enters at the first stop"
    );
    backend.inject(window, tab(false));
    assert_eq!(
        backend.focus_holder(window),
        Some(last),
        "skipping non-stops and hidden nodes"
    );
    backend.inject(window, tab(false));
    assert_eq!(backend.focus_holder(window), Some(first), "wraps forwards");
    backend.inject(window, tab(true));
    assert_eq!(
        backend.focus_holder(window),
        Some(last),
        "Shift+Tab wraps backwards"
    );
    assert!(
        !log.borrow()
            .iter()
            .any(|(_, event)| matches!(event, Event::KeyDown { .. })),
        "the focus moved, so no widget saw the Tab"
    );
}

#[test]
fn a_node_that_wants_tab_keeps_it() {
    let backend = OffscreenBackend::new();
    let window = backend.open_window(&PlatformSpec::new("tab")).unwrap();
    let editor = node(&backend, window, spec().tab_stop().wants_tab());
    let _other = node(&backend, window, spec().tab_stop());
    backend.focus(editor);
    let log = Rc::new(RefCell::new(Vec::new()));
    backend.set_event_sink(window, Rc::new(Recorder(Rc::clone(&log))));

    backend.inject(window, tab(false));
    assert_eq!(backend.focus_holder(window), Some(editor));
    assert!(
        matches!(log.borrow().last(), Some((target, Event::KeyDown { .. })) if *target == editor)
    );
}

fn key_down(key: xui_core::message::Key, modifiers: Modifiers) -> Event {
    Event::KeyDown {
        key,
        modifiers,
        repeat: 1,
        system: false,
    }
}

fn sunk(
    backend: &OffscreenBackend,
    window: xui_core::backend::WindowId,
) -> Rc<RefCell<Vec<(WidgetId, Event)>>> {
    let log = Rc::new(RefCell::new(Vec::new()));
    backend.set_event_sink(window, Rc::new(Recorder(Rc::clone(&log))));
    log
}

#[test]
fn shift_tab_from_no_focus_enters_at_the_last_stop() {
    let backend = OffscreenBackend::new();
    let window = backend.open_window(&PlatformSpec::new("tab")).unwrap();
    let _first = node(&backend, window, spec().tab_stop());
    let last = node(&backend, window, spec().tab_stop());
    backend.inject(window, tab(true));
    assert_eq!(backend.focus_holder(window), Some(last));
}

#[test]
fn disabled_and_covered_stops_are_skipped() {
    let backend = OffscreenBackend::new();
    let window = backend.open_window(&PlatformSpec::new("tab")).unwrap();
    let first = node(&backend, window, spec().tab_stop());
    let disabled = node(&backend, window, spec().tab_stop());
    backend.set_enabled(disabled, false);
    let parent = node(
        &backend,
        window,
        NodeSpec::new(NodeKind::Container, Rect::new(0, 30, 40, 50)),
    );
    let child = backend
        .create(ParentRef::Widget(parent), &spec().tab_stop())
        .unwrap();
    backend.set_enabled(parent, false);
    let covered = node(
        &backend,
        window,
        NodeSpec::new(NodeKind::Edit, Rect::new(0, 60, 40, 80)).tab_stop(),
    );
    // A scrim added later paints over `covered`, as an open dialog's does.
    let _scrim = node(
        &backend,
        window,
        NodeSpec::new(NodeKind::Container, Rect::new(0, 55, 100, 100)),
    );
    let _ = (child, covered);

    backend.inject(window, tab(false));
    assert_eq!(backend.focus_holder(window), Some(first));
    backend.inject(window, tab(false));
    assert_eq!(
        backend.focus_holder(window),
        Some(first),
        "the disabled, disabled-parent and covered stops leave only the first"
    );
}

#[test]
fn tab_with_no_stops_reaches_the_sink_as_a_key() {
    let backend = OffscreenBackend::new();
    let window = backend.open_window(&PlatformSpec::new("tab")).unwrap();
    let _label = node(&backend, window, spec());
    let log = sunk(&backend, window);
    backend.inject(window, tab(false));
    assert!(matches!(
        log.borrow().last(),
        Some((_, Event::KeyDown { .. }))
    ));
    assert_eq!(backend.focus_holder(window), None);
}

#[test]
fn a_popup_holder_and_modified_tabs_keep_the_key() {
    let backend = OffscreenBackend::new();
    let window = backend.open_window(&PlatformSpec::new("tab")).unwrap();
    let popup = node(&backend, window, spec().popup());
    let field = node(&backend, window, spec().tab_stop());
    let log = sunk(&backend, window);

    backend.focus(popup);
    backend.inject(window, tab(false));
    assert_eq!(
        backend.focus_holder(window),
        Some(popup),
        "a popup keeps Tab"
    );

    backend.focus(field);
    for modifiers in [
        Modifiers {
            ctrl: true,
            ..Modifiers::NONE
        },
        Modifiers {
            alt: true,
            ..Modifiers::NONE
        },
    ] {
        backend.inject(window, key_down(xui_core::message::Key::TAB, modifiers));
        assert_eq!(
            backend.focus_holder(window),
            Some(field),
            "{modifiers:?} is not traversal"
        );
        assert!(
            matches!(log.borrow().last(), Some((target, Event::KeyDown { .. })) if *target == field)
        );
    }
}

#[test]
fn a_holder_that_is_not_a_stop_continues_from_the_ends() {
    let backend = OffscreenBackend::new();
    let window = backend.open_window(&PlatformSpec::new("tab")).unwrap();
    let first = node(&backend, window, spec().tab_stop());
    let label = node(&backend, window, spec());
    let last = node(&backend, window, spec().tab_stop());
    backend.focus(label);
    backend.inject(window, tab(false));
    assert_eq!(backend.focus_holder(window), Some(first));
    backend.focus(label);
    backend.inject(window, tab(true));
    assert_eq!(backend.focus_holder(window), Some(last));
}

#[test]
fn stops_of_another_window_are_not_visited() {
    let backend = OffscreenBackend::new();
    let one = backend.open_window(&PlatformSpec::new("one")).unwrap();
    let two = backend.open_window(&PlatformSpec::new("two")).unwrap();
    let mine = node(&backend, one, spec().tab_stop());
    let _theirs = node(&backend, two, spec().tab_stop());
    backend.inject(one, tab(false));
    backend.inject(one, tab(false));
    assert_eq!(backend.focus_holder(one), Some(mine));
}
