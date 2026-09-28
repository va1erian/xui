use std::cell::RefCell;
use std::rc::Rc;

use super::MultilineEdit;
use crate::app::{App, Core, Runtime, Ui};
use crate::backend::headless::HeadlessBackend;
use crate::backend::{Backend, Event, PlatformSpec, WidgetId};
use crate::geometry::Rect;
use crate::message::{Key, Modifiers};
use crate::widget::HasText;

struct TestApp(Rc<RefCell<Vec<u32>>>);

impl App for TestApp {
    type Msg = u32;
    fn update(&mut self, msg: u32, _ui: &mut Ui<u32>) {
        self.0.borrow_mut().push(msg);
    }
}

fn setup() -> (Rc<Core<u32>>, Ui<u32>) {
    let backend = Rc::new(HeadlessBackend::new());
    let window = backend.open_window(&PlatformSpec::new("test")).unwrap();
    let core = Core::new(backend, window);
    let ui = Ui::new(Rc::clone(&core));
    (core, ui)
}

fn area(ui: &Ui<u32>, text: &str) -> MultilineEdit<u32> {
    MultilineEdit::new(ui, Rect::new(0, 0, 200, 100), text).unwrap()
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
fn typing_a_newline_changes_the_text_and_raises_messages() {
    let (core, ui) = setup();
    let log = Rc::new(RefCell::new(Vec::new()));
    let edit = area(&ui, "").on_change(|text| Some(text.chars().count() as u32));
    let runtime = Runtime::primary(core, TestApp(Rc::clone(&log)));

    runtime.deliver(edit.id(), &Event::SetFocus);
    runtime.deliver(edit.id(), &Event::Char('a'));
    runtime.deliver(edit.id(), &key(Key::RETURN));
    runtime.deliver(edit.id(), &Event::Char('b'));
    runtime.deliver(WidgetId::NONE, &Event::Wake);

    assert_eq!(edit.text(), "a\nb");
    assert_eq!(*log.borrow(), vec![1, 2, 3]);
}

#[test]
fn backspace_removes_the_newline_before_the_caret() {
    let (core, ui) = setup();
    let edit = area(&ui, "a\n");
    let runtime = Runtime::primary(core, TestApp(Rc::new(RefCell::new(Vec::new()))));

    runtime.deliver(edit.id(), &Event::SetFocus);
    runtime.deliver(edit.id(), &key(Key::BACK));

    assert_eq!(edit.text(), "a");
}

#[test]
fn set_text_changes_the_text_without_raising_a_message() {
    let (_core, ui) = setup();
    let log = Rc::new(RefCell::new(Vec::new()));
    let captured = Rc::clone(&log);
    let edit = area(&ui, "one").on_change(move |text| {
        captured.borrow_mut().push(text.to_string());
        None
    });

    edit.set_text("two\nlines");

    assert_eq!(edit.text(), "two\nlines");
    assert!(log.borrow().is_empty(), "set_text raises nothing");
}
