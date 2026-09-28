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
