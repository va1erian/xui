use std::cell::RefCell;
use std::rc::Rc;

use super::{Button, CheckBox, Control, Edit, HasText, Label, ProgressBar, RadioGroup, Slider};
use crate::app::{App, Core, Runtime, Ui};
use crate::backend::headless::{DrawOp, HeadlessBackend};
use crate::backend::{Backend, Event, NodeKind, NodeSpec, PlatformSpec, WidgetId};
use crate::geometry::Rect;
use crate::message::{Key, Modifiers, MouseButton};
use crate::property::{Properties, Value};

struct TestApp {
    log: Rc<RefCell<Vec<u32>>>,
}

impl App for TestApp {
    type Msg = u32;

    fn update(&mut self, msg: u32, _ui: &mut Ui<u32>) {
        self.log.borrow_mut().push(msg);
    }
}

fn setup() -> (Rc<HeadlessBackend>, Rc<Core<u32>>, Ui<u32>) {
    let backend = Rc::new(HeadlessBackend::new());
    let window = backend.open_window(&PlatformSpec::new("test")).unwrap();
    let core = Core::new(backend.clone(), window);
    let ui = Ui::new(Rc::clone(&core));
    (backend, core, ui)
}

fn click(runtime: &Runtime<TestApp>, target: WidgetId) {
    let modifiers = Modifiers::NONE;
    runtime.deliver(
        target,
        &Event::MouseDown {
            x: 5,
            y: 5,
            button: MouseButton::Left,
            modifiers,
        },
    );
    // A pointer that jitters between press and release must still click.
    runtime.deliver(
        target,
        &Event::MouseMove {
            x: 6,
            y: 5,
            modifiers,
        },
    );
    runtime.deliver(
        target,
        &Event::MouseUp {
            x: 5,
            y: 5,
            button: MouseButton::Left,
            modifiers,
        },
    );
    runtime.deliver(WidgetId::NONE, &Event::Wake);
}

#[test]
fn a_label_paints_its_text_from_the_theme() {
    let (backend, _core, ui) = setup();
    let label = Label::new(&ui, Rect::new(0, 0, 120, 30), "hello").unwrap();

    backend.render(label.id());
    let ops = backend.ops(label.id());
    assert!(
        ops.iter()
            .any(|op| matches!(op, DrawOp::Text(_, text, _) if text == "hello")),
        "the label painted its text: {ops:?}"
    );
}

#[test]
fn changing_a_labels_text_repaints_it() {
    let (backend, _core, ui) = setup();
    let label = Label::new(&ui, Rect::new(0, 0, 120, 30), "one").unwrap();
    label.set_text("two");

    backend.render(label.id());
    let ops = backend.ops(label.id());
    assert!(
        ops.iter()
            .any(|op| matches!(op, DrawOp::Text(_, text, _) if text == "two")),
        "the new text was painted: {ops:?}"
    );
}

#[test]
fn a_click_maps_to_the_apps_message() {
    let (_backend, core, ui) = setup();
    let button = Button::new(&ui, Rect::new(0, 0, 80, 28), "Ok")
        .unwrap()
        .on_click(|| Some(7));
    let log = Rc::new(RefCell::new(Vec::new()));
    let runtime = Runtime::primary(
        core,
        TestApp {
            log: Rc::clone(&log),
        },
    );

    click(&runtime, button.id());
    assert_eq!(*log.borrow(), vec![7]);
}

#[test]
fn a_disabled_button_ignores_clicks() {
    let (_backend, core, ui) = setup();
    let button = Button::new(&ui, Rect::new(0, 0, 80, 28), "Ok")
        .unwrap()
        .on_click(|| Some(7));
    button.set_enabled(false);
    let log = Rc::new(RefCell::new(Vec::new()));
    let runtime = Runtime::primary(
        core,
        TestApp {
            log: Rc::clone(&log),
        },
    );

    click(&runtime, button.id());
    assert!(log.borrow().is_empty(), "a disabled button raised nothing");
}

fn key(key: Key) -> Event {
    Event::KeyDown {
        key,
        modifiers: Modifiers::NONE,
        repeat: 1,
        system: false,
    }
}

#[test]
fn typing_into_an_edit_changes_it_and_maps_a_message() {
    let (_backend, core, ui) = setup();
    let edit = Edit::new(&ui, Rect::new(0, 0, 160, 28), "")
        .unwrap()
        .on_change(|text| Some(text.len() as u32));
    let log = Rc::new(RefCell::new(Vec::new()));
    let runtime = Runtime::primary(
        core,
        TestApp {
            log: Rc::clone(&log),
        },
    );

    runtime.deliver(edit.id(), &Event::SetFocus);
    runtime.deliver(edit.id(), &Event::Char('a'));
    runtime.deliver(edit.id(), &Event::Char('b'));
    runtime.deliver(WidgetId::NONE, &Event::Wake);

    assert_eq!(edit.text(), "ab");
    assert_eq!(*log.borrow(), vec![1, 2]);
}

#[test]
fn editing_keys_move_the_caret_and_delete() {
    let (_backend, core, ui) = setup();
    let edit = Edit::new(&ui, Rect::new(0, 0, 160, 28), "abc").unwrap();
    let runtime = Runtime::primary(
        core,
        TestApp {
            log: Rc::new(RefCell::new(Vec::new())),
        },
    );

    runtime.deliver(edit.id(), &Event::SetFocus);
    runtime.deliver(edit.id(), &key(Key::HOME));
    runtime.deliver(edit.id(), &key(Key::DELETE));
    assert_eq!(
        edit.text(),
        "bc",
        "Delete removes the character at the caret"
    );

    runtime.deliver(edit.id(), &key(Key::END));
    runtime.deliver(edit.id(), &key(Key::BACK));
    assert_eq!(edit.text(), "b", "Backspace removes before the caret");
}

#[test]
fn an_edit_paints_its_field_and_text() {
    let (backend, _core, ui) = setup();
    let edit = Edit::new(&ui, Rect::new(0, 0, 160, 28), "typed").unwrap();

    backend.render(edit.id());
    let ops = backend.ops(edit.id());
    assert!(
        ops.iter().any(|op| matches!(op, DrawOp::Stroke(..))),
        "a field border was painted: {ops:?}"
    );
    assert!(
        ops.iter()
            .any(|op| matches!(op, DrawOp::Text(_, text, _) if text == "typed")),
        "the text was painted: {ops:?}"
    );
}

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
fn a_button_paints_a_face_and_its_label() {
    let (backend, _core, ui) = setup();
    let button = Button::new(&ui, Rect::new(0, 0, 80, 28), "Send").unwrap();

    backend.render(button.id());
    let ops = backend.ops(button.id());
    assert!(
        ops.iter().any(|op| matches!(op, DrawOp::Rounded(..))),
        "a button face was painted: {ops:?}"
    );
    assert!(
        ops.iter()
            .any(|op| matches!(op, DrawOp::Text(_, text, _) if text == "Send")),
        "the button label was painted: {ops:?}"
    );
}
