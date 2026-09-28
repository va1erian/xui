use std::cell::RefCell;
use std::rc::Rc;

use super::{Button, CheckBox, Control, Edit, HasText, Label, ProgressBar, RadioGroup, Slider};
use crate::app::{App, Core, Runtime, Ui};
use crate::backend::headless::{DrawOp, HeadlessBackend};
use crate::backend::{Backend, Event, NodeKind, NodeSpec, PlatformSpec, WidgetId};
use crate::geometry::Rect;
use crate::message::{Key, Modifiers, MouseButton};
use crate::property::{Properties, Value};

mod button;
mod checkbox;
mod edit;
mod label;
mod misc;
mod slider;

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

fn key(key: Key) -> Event {
    Event::KeyDown {
        key,
        modifiers: Modifiers::NONE,
        repeat: 1,
        system: false,
    }
}
