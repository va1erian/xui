use super::*;

#[test]
fn a_progress_bar_clamps_and_reports_its_value() {
    let (_backend, _core, ui) = setup();
    let bar = ProgressBar::new(&ui, Rect::new(0, 0, 200, 8), 10).unwrap();
    assert_eq!(bar.value(), 0);
    bar.set_value(4);
    assert_eq!(bar.value(), 4);
    bar.set_value(99);
    assert_eq!(bar.value(), 10, "clamped to max");
    bar.set_max(5);
    assert_eq!(bar.value(), 5);
    assert_eq!(bar.property("value"), Some(Value::Integer(5)));
}

#[test]
fn a_slider_drags_and_commits() {
    let (_backend, core, ui) = setup();
    let changes = Rc::new(RefCell::new(Vec::new()));
    let commits = Rc::new(RefCell::new(Vec::new()));
    let on_change = Rc::clone(&changes);
    let on_commit = Rc::clone(&commits);
    let slider = Slider::new(&ui, Rect::new(0, 0, 200, 20), 0.0, 100.0)
        .unwrap()
        .on_change(move |v| {
            on_change.borrow_mut().push(v);
            None
        })
        .on_commit(move |v| {
            on_commit.borrow_mut().push(v);
            None
        });
    let runtime = Runtime::primary(
        core,
        TestApp {
            log: Rc::new(RefCell::new(Vec::new())),
        },
    );

    let modifiers = Modifiers::NONE;
    runtime.deliver(
        slider.id(),
        &Event::MouseDown {
            x: 100,
            y: 10,
            button: MouseButton::Left,
            modifiers,
        },
    );
    runtime.deliver(
        slider.id(),
        &Event::MouseMove {
            x: 192,
            y: 10,
            modifiers,
        },
    );
    runtime.deliver(
        slider.id(),
        &Event::MouseUp {
            x: 192,
            y: 10,
            button: MouseButton::Left,
            modifiers,
        },
    );

    assert!((slider.value() - 100.0).abs() < 0.001, "dragged to the end");
    assert_eq!(changes.borrow().len(), 2, "two changes while dragging");
    assert_eq!(commits.borrow().len(), 1, "one commit on release");
}

#[test]
fn a_slider_steps_with_the_keyboard_and_clamps() {
    let (_backend, core, ui) = setup();
    let slider = Slider::new(&ui, Rect::new(0, 0, 200, 20), 0.0, 100.0).unwrap();
    let runtime = Runtime::primary(
        core,
        TestApp {
            log: Rc::new(RefCell::new(Vec::new())),
        },
    );

    runtime.deliver(slider.id(), &key(Key::RIGHT));
    assert!((slider.value() - 5.0).abs() < 1e-6, "one step is range/20");
    runtime.deliver(slider.id(), &key(Key::END));
    assert_eq!(slider.value(), 100.0);
    runtime.deliver(slider.id(), &key(Key::LEFT));
    assert!((slider.value() - 95.0).abs() < 1e-6);

    slider.set_range(0.0, 10.0);
    assert_eq!(
        slider.value(),
        10.0,
        "the value is clamped to the new range"
    );
    assert_eq!(slider.property("value"), Some(Value::Float(10.0)));
}
