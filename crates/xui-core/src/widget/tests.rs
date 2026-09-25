use std::cell::RefCell;
use std::rc::Rc;

use super::{Button, HasText, Label};
use crate::app::{App, Core, Runtime, Ui};
use crate::backend::headless::{DrawOp, HeadlessBackend};
use crate::backend::{Backend, Event, PlatformSpec, WidgetId};
use crate::geometry::Rect;
use crate::message::{Modifiers, MouseButton};

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
