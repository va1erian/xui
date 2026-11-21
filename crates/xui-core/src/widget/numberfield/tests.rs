use super::NumberField;
use crate::app::{Core, Ui};
use crate::backend::headless::HeadlessBackend;
use crate::backend::{Backend, Event, PlatformSpec};
use crate::geometry::Rect;
use crate::message::{Key, Modifiers, MouseButton};
use std::cell::RefCell;
use std::rc::Rc;

fn harness() -> (Rc<Core<()>>, Ui<()>) {
    let backend = Rc::new(HeadlessBackend::new());
    let window = backend.open_window(&PlatformSpec::new("test")).unwrap();
    let core = Core::new(backend, window);
    (Rc::clone(&core), Ui::new(Rc::clone(&core)))
}

fn number(ui: &Ui<()>, max: f64) -> NumberField<()> {
    NumberField::new(ui, Rect::new(0, 0, 160, 28), 0.0, max, 5.0).unwrap()
}
fn record(log: &Rc<RefCell<Vec<f64>>>, value: f64) -> Option<()> {
    log.borrow_mut().push(value);
    None
}
fn key(k: Key) -> Event {
    Event::KeyDown {
        key: k,
        modifiers: Modifiers::NONE,
        repeat: 1,
        system: false,
    }
}
fn down(x: i32, y: i32) -> Event {
    Event::MouseDown {
        x,
        y,
        button: MouseButton::Left,
        modifiers: Modifiers::NONE,
    }
}

#[test]
fn clicking_the_plus_stepper_commits_the_next_value() {
    let (core, ui) = harness();
    let seen = Rc::new(RefCell::new(Vec::new()));
    let log = Rc::clone(&seen);
    let field = number(&ui, 100.0).on_commit(move |v| record(&log, v));
    core.router().dispatch(field.id(), &down(150, 5));
    assert_eq!((field.value(), seen.borrow().clone()), (5.0, vec![5.0]));
}

#[test]
fn typing_then_return_commits_and_clamps_to_max() {
    let (core, ui) = harness();
    let seen = Rc::new(RefCell::new(Vec::new()));
    let log = Rc::clone(&seen);
    let field = number(&ui, 50.0).on_commit(move |v| record(&log, v));
    core.router().dispatch(field.id(), &Event::SetFocus);
    for c in ['9', '9'] {
        core.router().dispatch(field.id(), &Event::Char(c));
    }
    core.router().dispatch(field.id(), &key(Key::RETURN));
    assert_eq!((field.value(), seen.borrow().clone()), (50.0, vec![50.0]));
}

#[test]
fn set_value_raises_nothing() {
    let (_core, ui) = harness();
    let seen = Rc::new(RefCell::new(Vec::new()));
    let (a, b) = (Rc::clone(&seen), Rc::clone(&seen));
    let field = number(&ui, 100.0)
        .on_change(move |v| record(&a, v))
        .on_commit(move |v| record(&b, v));
    field.set_value(42.0);
    assert_eq!(field.value(), 42.0);
    assert!(seen.borrow().is_empty(), "set_value raised nothing");
}
