use super::*;

#[test]
fn a_checkbox_toggles_with_a_click_or_space() {
    let (_backend, core, ui) = setup();
    let checkbox = CheckBox::new(&ui, Rect::new(0, 0, 160, 28), "Agree")
        .unwrap()
        .on_toggle(|checked| Some(checked as u32));
    let log = Rc::new(RefCell::new(Vec::new()));
    let runtime = Runtime::primary(
        core,
        TestApp {
            log: Rc::clone(&log),
        },
    );

    assert!(!checkbox.is_checked());
    click(&runtime, checkbox.id());
    assert!(checkbox.is_checked());
    runtime.deliver(checkbox.id(), &key(Key::SPACE));
    runtime.deliver(WidgetId::NONE, &Event::Wake);
    assert!(!checkbox.is_checked(), "Space toggles it back");
    assert_eq!(*log.borrow(), vec![1, 0]);
}

#[test]
fn a_disabled_checkbox_ignores_input() {
    let (_backend, core, ui) = setup();
    let checkbox = CheckBox::new(&ui, Rect::new(0, 0, 160, 28), "Agree")
        .unwrap()
        .on_toggle(|checked| Some(checked as u32));
    checkbox.set_enabled(false);
    let log = Rc::new(RefCell::new(Vec::new()));
    let runtime = Runtime::primary(
        core,
        TestApp {
            log: Rc::clone(&log),
        },
    );

    click(&runtime, checkbox.id());
    runtime.deliver(checkbox.id(), &key(Key::SPACE));
    runtime.deliver(WidgetId::NONE, &Event::Wake);
    assert!(!checkbox.is_checked(), "a disabled box toggles nothing");
    assert!(log.borrow().is_empty());
}

#[test]
fn a_checkbox_ignores_key_auto_repeat() {
    let (_backend, core, ui) = setup();
    let checkbox = CheckBox::new(&ui, Rect::new(0, 0, 160, 28), "Agree").unwrap();
    let runtime = Runtime::primary(
        core,
        TestApp {
            log: Rc::new(RefCell::new(Vec::new())),
        },
    );

    runtime.deliver(
        checkbox.id(),
        &Event::KeyDown {
            key: Key::SPACE,
            modifiers: Modifiers::NONE,
            repeat: 3,
            system: false,
        },
    );
    assert!(!checkbox.is_checked(), "an auto-repeat does not toggle");
}

#[test]
fn a_checkbox_reports_its_checked_property() {
    let (_backend, _core, ui) = setup();
    let checkbox = CheckBox::new(&ui, Rect::new(0, 0, 160, 28), "Agree").unwrap();
    assert_eq!(checkbox.property("checked"), Some(Value::Bool(false)));
    checkbox.set_property("checked", Value::Bool(true));
    assert!(checkbox.is_checked());
}

fn ops(backend: &HeadlessBackend, checkbox: &CheckBox<u32>) -> Vec<DrawOp> {
    backend.render(checkbox.id());
    backend.ops(checkbox.id())
}

/// What a checked box paints with no animation at all.
fn settled_checked() -> Vec<DrawOp> {
    let (backend, _core, ui) = setup();
    let checkbox = CheckBox::new(&ui, Rect::new(0, 0, 160, 28), "Agree").unwrap();
    checkbox.set_checked(true);
    assert_eq!(backend.frame_requests(), 0);
    ops(&backend, &checkbox)
}

#[test]
fn a_toggle_draws_the_mark_in_and_settles_on_the_checked_picture() {
    use std::time::Duration;

    let (backend, core, ui) = setup();
    backend.set_now(Duration::ZERO);
    let checkbox = CheckBox::new(&ui, Rect::new(0, 0, 160, 28), "Agree").unwrap();
    let runtime = Runtime::primary(
        core,
        TestApp {
            log: Rc::new(RefCell::new(Vec::new())),
        },
    );
    let unchecked = ops(&backend, &checkbox);
    let checked = settled_checked();

    runtime.deliver(checkbox.id(), &key(Key::SPACE));
    assert!(checkbox.is_checked(), "the state changes at once");
    backend.advance(Duration::from_millis(20));
    runtime.deliver(WidgetId::NONE, &Event::Frame);
    let midway = ops(&backend, &checkbox);
    assert_ne!(midway, unchecked);
    assert_ne!(midway, checked);

    backend.advance(Duration::from_millis(200));
    runtime.deliver(WidgetId::NONE, &Event::Frame);
    assert_eq!(ops(&backend, &checkbox), checked);
    assert_eq!(backend.frame_requests(), 2, "and no frame asked for after");

    // Unchecking draws it back out to the plain box.
    runtime.deliver(checkbox.id(), &key(Key::SPACE));
    backend.advance(Duration::from_millis(20));
    runtime.deliver(WidgetId::NONE, &Event::Frame);
    assert_ne!(ops(&backend, &checkbox), unchecked);
    backend.advance(Duration::from_millis(200));
    runtime.deliver(WidgetId::NONE, &Event::Frame);
    assert_eq!(ops(&backend, &checkbox), unchecked);
}

#[test]
fn without_a_clock_a_toggle_paints_the_checked_picture_at_once() {
    let (backend, core, ui) = setup();
    let checkbox = CheckBox::new(&ui, Rect::new(0, 0, 160, 28), "Agree").unwrap();
    let runtime = Runtime::primary(
        core,
        TestApp {
            log: Rc::new(RefCell::new(Vec::new())),
        },
    );
    runtime.deliver(checkbox.id(), &key(Key::SPACE));
    assert_eq!(ops(&backend, &checkbox), settled_checked());
    assert_eq!(backend.frame_requests(), 0);
}
