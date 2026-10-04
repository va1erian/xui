use super::*;

#[test]
fn a_widget_event_is_mapped_to_a_message_and_delivered() {
    let (backend, window, core, ui) = setup();
    let node = backend
        .create(
            ParentRef::Window(window),
            &NodeSpec::new(NodeKind::Custom, Rect::default()),
        )
        .unwrap();
    ui.register_events(node, |event| match event {
        Event::Char('a') => Some(10),
        Event::Char('b') => Some(20),
        _ => None,
    });

    let log = Rc::new(RefCell::new(Vec::new()));
    let runtime = Runtime::primary(Rc::clone(&core), test_app(&log));

    // The backend would deliver these; drive the runtime directly.
    assert!(runtime.deliver(node, &Event::Char('a')));
    assert!(runtime.deliver(node, &Event::Char('b')));
    assert!(!runtime.deliver(node, &Event::Char('z')), "unmapped event");

    // The mapper enqueued and woke the backend; the wake drains.
    runtime.deliver(WidgetId::NONE, &Event::Wake);
    assert_eq!(*log.borrow(), vec![10, 20]);
}

#[test]
fn a_close_request_maps_to_a_message() {
    let (backend, window, core, ui) = setup();
    ui.on_close(|| Some(99));
    let log = Rc::new(RefCell::new(Vec::new()));
    let runtime = Runtime::primary(Rc::clone(&core), test_app(&log));

    assert!(runtime.deliver(WidgetId::NONE, &Event::Close));
    runtime.deliver(WidgetId::NONE, &Event::Wake);
    assert_eq!(*log.borrow(), vec![99]);
    assert!(
        backend.window_title(window).is_some(),
        "the window was kept open"
    );
}

#[test]
fn closing_without_a_mapping_closes_the_window_and_quits() {
    let (backend, window, core, _ui) = setup();
    let runtime = Runtime::primary(
        Rc::clone(&core),
        TestApp {
            log: Rc::new(RefCell::new(Vec::new())),
        },
    );

    runtime.deliver(WidgetId::NONE, &Event::Close);
    assert!(backend.window_title(window).is_none(), "the window closed");
    assert!(backend.quit_requested());
}

#[test]
fn a_secondary_window_closes_without_quitting() {
    let (backend, window, core, _ui) = setup();
    let runtime = Runtime::new(
        Rc::clone(&core),
        TestApp {
            log: Rc::new(RefCell::new(Vec::new())),
        },
        false,
    );

    runtime.deliver(WidgetId::NONE, &Event::Close);
    assert!(backend.window_title(window).is_none(), "the window closed");
    assert!(!backend.quit_requested(), "the loop keeps running");
}

#[test]
fn a_timer_tick_maps_to_a_message() {
    let (_backend, _window, core, ui) = setup();
    let id = TimerId(7);
    ui.on_timer(move |fired| (fired == id).then_some(5));
    let log = Rc::new(RefCell::new(Vec::new()));
    let runtime = Runtime::primary(Rc::clone(&core), test_app(&log));

    runtime.deliver(WidgetId::NONE, &Event::Timer { id });
    runtime.deliver(WidgetId::NONE, &Event::Wake);
    assert_eq!(*log.borrow(), vec![5]);
}

#[test]
fn a_dpi_change_maps_to_a_message_with_the_new_dpi() {
    let (_backend, _window, core, ui) = setup();
    let seen = Rc::new(Cell::new(None));
    let seen_for_callback = Rc::clone(&seen);
    ui.on_dpi_changed(move |dpi, suggested| {
        seen_for_callback.set(Some((dpi, suggested)));
        Some(dpi)
    });
    let log = Rc::new(RefCell::new(Vec::new()));
    let runtime = Runtime::primary(Rc::clone(&core), test_app(&log));

    let suggested = Rect::new(10, 20, 810, 620);
    assert!(runtime.deliver(
        WidgetId::NONE,
        &Event::DpiChanged {
            dpi: 192,
            suggested,
        }
    ));
    runtime.deliver(WidgetId::NONE, &Event::Wake);

    assert_eq!(*log.borrow(), vec![192]);
    assert_eq!(seen.get(), Some((192, suggested)));
}

#[test]
fn a_shortcut_key_maps_to_a_message_and_still_reaches_the_widget() {
    use crate::message::{Key, Modifiers};

    let (backend, window, core, ui) = setup();
    let node = backend
        .create(
            ParentRef::Window(window),
            &NodeSpec::new(NodeKind::Custom, Rect::default()),
        )
        .unwrap();
    let seen = Rc::new(Cell::new(0));
    let seen_for_widget = Rc::clone(&seen);
    ui.register_events(node, move |event| {
        if matches!(event, Event::KeyDown { .. }) {
            seen_for_widget.set(seen_for_widget.get() + 1);
        }
        None
    });
    ui.on_key(|key, modifiers| (key == Key::S && modifiers.ctrl).then_some(7));

    let log = Rc::new(RefCell::new(Vec::new()));
    let runtime = Runtime::primary(Rc::clone(&core), test_app(&log));
    let ctrl_s = Event::KeyDown {
        key: Key::S,
        modifiers: Modifiers {
            ctrl: true,
            ..Modifiers::NONE
        },
        repeat: 1,
        system: false,
    };
    runtime.deliver(node, &ctrl_s);
    runtime.deliver(WidgetId::NONE, &Event::Wake);

    assert_eq!(*log.borrow(), vec![7], "the shortcut was mapped");
    assert_eq!(seen.get(), 1, "the widget still saw the key");
}

#[test]
fn a_shortcut_is_handled_before_the_keys_typed_after_it() {
    use crate::message::{Key, Modifiers};

    let (backend, window, core, ui) = setup();
    let node = backend
        .create(
            ParentRef::Window(window),
            &NodeSpec::new(NodeKind::Custom, Rect::default()),
        )
        .unwrap();
    let log = Rc::new(RefCell::new(Vec::new()));
    // The widget records what the app had handled when each character
    // reached it.
    let typed = Rc::new(RefCell::new(Vec::new()));
    {
        let log = Rc::clone(&log);
        let typed = Rc::clone(&typed);
        ui.register_events(node, move |event| {
            if let Event::Char(c) = event {
                typed.borrow_mut().push((*c, log.borrow().len()));
            }
            None
        });
    }
    ui.on_key(|key, modifiers| (key == Key::L && modifiers.ctrl).then_some(7));

    let runtime = Runtime::primary(Rc::clone(&core), test_app(&log));
    runtime.deliver(
        node,
        &Event::KeyDown {
            key: Key::L,
            modifiers: Modifiers {
                ctrl: true,
                ..Modifiers::NONE
            },
            repeat: 1,
            system: false,
        },
    );
    // No wake in between: the backend had the character queued already.
    runtime.deliver(node, &Event::Char('h'));

    assert_eq!(*log.borrow(), vec![7]);
    assert_eq!(*typed.borrow(), vec![('h', 1)], "the shortcut ran first");
}

#[test]
fn a_key_the_shortcut_mapper_declines_stays_unmapped() {
    use crate::message::{Key, Modifiers};

    let (_backend, _window, core, ui) = setup();
    ui.on_key(|key, _| (key == Key::S).then_some(7));
    let log = Rc::new(RefCell::new(Vec::new()));
    let runtime = Runtime::primary(Rc::clone(&core), test_app(&log));

    runtime.deliver(
        WidgetId::NONE,
        &Event::KeyDown {
            key: Key::A,
            modifiers: Modifiers::NONE,
            repeat: 1,
            system: false,
        },
    );
    runtime.deliver(WidgetId::NONE, &Event::Wake);
    assert!(log.borrow().is_empty(), "only the mapped key raises");
}

#[test]
fn a_dpi_change_without_a_mapper_is_a_no_op() {
    let (_backend, _window, core, _ui) = setup();
    let log = Rc::new(RefCell::new(Vec::new()));
    let runtime = Runtime::primary(Rc::clone(&core), test_app(&log));

    assert!(runtime.deliver(
        WidgetId::NONE,
        &Event::DpiChanged {
            dpi: 192,
            suggested: Rect::default(),
        }
    ));
    runtime.deliver(WidgetId::NONE, &Event::Wake);
    assert!(log.borrow().is_empty(), "nothing was mapped");
}

#[test]
fn every_raises_its_message_on_its_own_timer_only() {
    let (_backend, _window, core, ui) = setup();
    ui.every(1000, 7);
    ui.every(500, 8);
    let log = Rc::new(RefCell::new(Vec::new()));
    let runtime = Runtime::primary(Rc::clone(&core), test_app(&log));

    // The headless backend numbers timers from one, in creation order.
    runtime.deliver(WidgetId::NONE, &Event::Timer { id: TimerId(2) });
    runtime.deliver(WidgetId::NONE, &Event::Timer { id: TimerId(1) });
    runtime.deliver(WidgetId::NONE, &Event::Timer { id: TimerId(1) });
    runtime.deliver(WidgetId::NONE, &Event::Wake);
    assert_eq!(*log.borrow(), vec![8, 7, 7]);
}
