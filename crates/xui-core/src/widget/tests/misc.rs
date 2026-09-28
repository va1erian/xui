use super::*;

#[test]
fn widgets_report_and_edit_properties() {
    let (_backend, _core, ui) = setup();
    let label = Label::new(&ui, Rect::new(0, 0, 120, 30), "hi").unwrap();
    assert_eq!(label.property("text"), Some(Value::Text("hi".to_string())));
    assert!(label.set_property("text", Value::Text("bye".to_string())));
    assert_eq!(label.text(), "bye");
    assert!(!label.set_property("nope", Value::Bool(true)));

    let button = Button::new(&ui, Rect::new(0, 0, 80, 28), "Ok").unwrap();
    assert_eq!(button.property("enabled"), Some(Value::Bool(true)));
    assert!(button.set_property("enabled", Value::Bool(false)));
    assert!(!button.is_enabled());
}

#[test]
fn design_mode_suppresses_widget_input() {
    let (_backend, core, ui) = setup();
    let button = Button::new(&ui, Rect::new(0, 0, 80, 28), "Ok")
        .unwrap()
        .on_click(|| Some(1));
    let log = Rc::new(RefCell::new(Vec::new()));
    let runtime = Runtime::primary(
        core,
        TestApp {
            log: Rc::clone(&log),
        },
    );

    ui.set_design_mode(true);
    click(&runtime, button.id());
    assert!(
        log.borrow().is_empty(),
        "a widget ignores input in design mode"
    );

    ui.set_design_mode(false);
    click(&runtime, button.id());
    assert_eq!(*log.borrow(), vec![1]);
}

#[test]
fn selecting_a_widget_paints_an_outline() {
    let (backend, _core, ui) = setup();
    let button = Button::new(&ui, Rect::new(0, 0, 80, 28), "Ok").unwrap();
    button.set_selected(true);

    backend.render(button.id());
    let accent = ui.theme().accent;
    assert!(
        backend
            .ops(button.id())
            .iter()
            .any(|op| matches!(op, DrawOp::Stroke(_, color, _) if *color == accent)),
        "a selected widget draws an accent outline"
    );
}

#[test]
fn a_controls_bounds_round_trip() {
    let (_backend, _core, ui) = setup();
    let control = Control::new(
        &ui,
        &NodeSpec::new(NodeKind::Label, Rect::new(0, 0, 10, 10)),
    )
    .unwrap();
    control.set_bounds(Rect::new(5, 6, 50, 60));
    assert_eq!(control.bounds(), Rect::new(5, 6, 50, 60));
}

#[test]
fn a_radio_group_selects_one_option() {
    let (_backend, core, ui) = setup();
    let group = RadioGroup::new(&ui, Rect::new(0, 0, 200, 84), &["Small", "Medium", "Large"])
        .unwrap()
        .on_select(|index| Some(index as u32));
    let ids = group.ids();
    let log = Rc::new(RefCell::new(Vec::new()));
    let runtime = Runtime::primary(
        core,
        TestApp {
            log: Rc::clone(&log),
        },
    );

    assert_eq!(group.selected(), 0);
    let modifiers = Modifiers::NONE;
    runtime.deliver(
        ids[2],
        &Event::MouseUp {
            x: 5,
            y: 5,
            button: MouseButton::Left,
            modifiers,
        },
    );
    runtime.deliver(WidgetId::NONE, &Event::Wake);
    assert_eq!(group.selected(), 2);
    assert_eq!(*log.borrow(), vec![2]);
    assert_eq!(group.property("selected"), Some(Value::Integer(2)));

    group.select(1);
    assert_eq!(group.selected(), 1, "programmatic select raises nothing");
    assert_eq!(*log.borrow(), vec![2]);
}

#[test]
fn dropping_a_widget_unregisters_its_event_mapper() {
    let (_backend, core, ui) = setup();
    let button = Button::new(&ui, Rect::new(0, 0, 80, 28), "Ok").unwrap();
    assert!(!core.router().is_empty(), "the mapper is registered");

    drop(button);
    assert!(
        core.router().is_empty(),
        "the mapper was unregistered, breaking the Core cycle"
    );
}
