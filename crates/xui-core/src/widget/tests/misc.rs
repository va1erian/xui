use super::*;
use crate::Panel;

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
fn widgets_report_and_edit_visibility_and_enabled() {
    use crate::widget::{ComboBox, Toolbar};

    let (_backend, _core, ui) = setup();
    let rect = Rect::new(0, 0, 120, 30);
    let slider = Slider::new(&ui, rect, 0.0, 10.0).unwrap();
    let combo = ComboBox::new(&ui, rect, &["a", "b"]).unwrap();
    let panel = Panel::new(&ui, rect).unwrap();
    let label = Label::new(&ui, rect, "hi").unwrap();
    let radios = RadioGroup::new(&ui, rect, &["a", "b"]).unwrap();
    let toolbar = Toolbar::new(&ui, rect, &["a"]).unwrap();

    let widgets: [(&dyn Properties, WidgetId, bool); 6] = [
        (&slider, slider.id(), true),
        (&combo, combo.id(), true),
        (&panel, panel.id(), true),
        (&label, label.id(), false),
        (&radios, radios.ids()[1], true),
        (&toolbar, toolbar.id(), true),
    ];
    for (widget, id, has_enabled) in widgets {
        assert_eq!(widget.property("visible"), Some(Value::Bool(true)));
        assert!(widget.set_property("visible", Value::Bool(false)));
        assert!(!ui.is_visible(id));
        assert_eq!(widget.property("visible"), Some(Value::Bool(false)));
        assert!(widget.set_property("visible", Value::Bool(true)));
        assert!(ui.is_visible(id));

        assert_eq!(widget.property("enabled").is_some(), has_enabled);
        if has_enabled {
            assert_eq!(widget.property("enabled"), Some(Value::Bool(true)));
            assert!(widget.set_property("enabled", Value::Bool(false)));
            assert_eq!(widget.property("enabled"), Some(Value::Bool(false)));
            assert!(!ui.is_enabled(id));
        } else {
            assert!(!widget.set_property("enabled", Value::Bool(false)));
        }
    }
}

#[test]
fn node_properties_reject_mismatched_values_and_empty_groups() {
    let (_backend, _core, ui) = setup();
    let rect = Rect::new(0, 0, 120, 30);
    let slider = Slider::new(&ui, rect, 0.0, 10.0).unwrap();
    assert!(!slider.set_property("visible", Value::Integer(0)));
    assert!(!slider.set_property("enabled", Value::Text("no".into())));
    assert!(ui.is_visible(slider.id()) && ui.is_enabled(slider.id()));

    let empty = RadioGroup::new(&ui, rect, &[]).unwrap();
    assert_eq!(empty.property("visible"), None);
    assert!(!empty.set_property("visible", Value::Bool(false)));
    assert!(!empty.set_property("enabled", Value::Bool(false)));
}

#[test]
fn destroying_a_node_forgets_that_it_was_disabled() {
    let (_backend, _core, ui) = setup();
    let slider = Slider::new(&ui, Rect::new(0, 0, 120, 30), 0.0, 10.0).unwrap();
    let id = slider.id();
    slider.set_enabled(false);
    assert!(!ui.is_enabled(id));
    drop(slider);
    assert!(
        ui.is_enabled(id),
        "a destroyed node leaves no record behind"
    );
}

#[test]
fn schema_lists_every_property_of_an_overriding_widget() {
    let (_backend, _core, ui) = setup();
    let label = Label::new(&ui, Rect::new(0, 0, 120, 30), "hi").unwrap();
    let schema: Vec<_> = label.schema().iter().map(|info| info.name).collect();
    let props: Vec<_> = label.properties().iter().map(|p| p.name).collect();
    assert_eq!(schema, props);
    assert!(schema.contains(&"visible"));
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

#[test]
fn design_mode_is_scoped_to_a_container() {
    let (_backend, core, ui) = setup();
    let panel = Panel::new(&ui, Rect::new(0, 0, 200, 100)).unwrap();
    let inner = Panel::new(panel.ui(), Rect::new(0, 0, 100, 50)).unwrap();
    let inside = Button::new(panel.ui(), Rect::new(0, 0, 80, 28), "In")
        .unwrap()
        .on_click(|| Some(1));
    let nested = Button::new(inner.ui(), Rect::new(0, 0, 40, 20), "Deep")
        .unwrap()
        .on_click(|| Some(2));
    let outside = Button::new(&ui, Rect::new(0, 200, 80, 28), "Out")
        .unwrap()
        .on_click(|| Some(3));
    let log = Rc::new(RefCell::new(Vec::new()));
    let runtime = Runtime::primary(
        core,
        TestApp {
            log: Rc::clone(&log),
        },
    );

    panel.set_design_mode(true);
    assert!(panel.ui().is_design_mode() && inner.ui().is_design_mode());
    assert!(!ui.is_design_mode());
    click(&runtime, inside.id());
    click(&runtime, nested.id());
    assert!(log.borrow().is_empty(), "the panel's subtree ignores input");
    click(&runtime, outside.id());
    assert_eq!(*log.borrow(), vec![3], "a sibling outside stays live");

    panel.set_design_mode(false);
    click(&runtime, inside.id());
    click(&runtime, nested.id());
    assert_eq!(*log.borrow(), vec![3, 1, 2]);

    inner.set_design_mode(true);
    click(&runtime, inside.id());
    click(&runtime, nested.id());
    assert_eq!(
        *log.borrow(),
        vec![3, 1, 2, 1],
        "only the inner panel is off"
    );
}

#[test]
fn window_design_mode_reaches_scoped_containers() {
    let (_backend, core, ui) = setup();
    let panel = Panel::new(&ui, Rect::new(0, 0, 200, 100)).unwrap();
    let inside = Button::new(panel.ui(), Rect::new(0, 0, 80, 28), "In")
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
    click(&runtime, inside.id());
    assert!(log.borrow().is_empty());
    ui.set_design_mode(false);
    click(&runtime, inside.id());
    assert_eq!(*log.borrow(), vec![1]);
}

fn container(ui: &Ui<u32>) -> Control<u32> {
    Control::new(
        ui,
        &NodeSpec::new(NodeKind::Container, Rect::new(0, 0, 80, 28)),
    )
    .unwrap()
}

#[test]
fn a_control_receives_only_its_own_timer_ticks() {
    let (_backend, core, ui) = setup();
    let button = container(&ui);
    let other = container(&ui);
    let log = Rc::new(RefCell::new(Vec::new()));
    let runtime = Runtime::primary(
        core,
        TestApp {
            log: Rc::clone(&log),
        },
    );
    let tick = |id| {
        runtime.deliver(WidgetId::NONE, &Event::Timer { id });
        runtime.deliver(WidgetId::NONE, &Event::Wake);
    };
    ui.on_timer(|_| Some(99));

    let mine = button.set_timer(500, || Some(1)).unwrap();
    let theirs = other.set_timer(500, || Some(2)).unwrap();
    assert_ne!(mine, theirs);
    tick(mine);
    tick(theirs);
    assert_eq!(*log.borrow(), vec![99, 1, 99, 2], "the app mapping is kept");

    button.kill_timer(mine);
    log.borrow_mut().clear();
    tick(mine);
    assert_eq!(*log.borrow(), vec![99], "a killed timer no longer ticks");
}

#[test]
fn dropping_a_control_stops_its_timers() {
    let (_backend, core, ui) = setup();
    let log = Rc::new(RefCell::new(Vec::new()));
    let runtime = Runtime::primary(
        core,
        TestApp {
            log: Rc::clone(&log),
        },
    );
    let timer = { container(&ui).set_timer(500, || Some(1)).unwrap() };
    runtime.deliver(WidgetId::NONE, &Event::Timer { id: timer });
    runtime.deliver(WidgetId::NONE, &Event::Wake);
    assert!(log.borrow().is_empty());
}
