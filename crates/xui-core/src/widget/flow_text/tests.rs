#![forbid(unsafe_code)]

//! Tests for the portable [`FlowText`](super::FlowText): that runs paint and a
//! link click reaches the app.

use std::cell::RefCell;
use std::rc::Rc;

use super::{FlowText, Run};
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
    let window = backend.open_window(&PlatformSpec::new("t")).unwrap();
    let core = Core::new(backend.clone(), window);
    let ui = Ui::new(Rc::clone(&core));
    (backend, core, ui)
}

fn line(ui: &Ui<u32>) -> FlowText<u32> {
    FlowText::new(ui, Rect::new(0, 0, 200, 40))
        .unwrap()
        .run(Run::link("one").on_click(|| Some(7)))
        .separator(" ")
        .run(Run::normal("two"))
}

#[test]
fn every_run_is_painted() {
    let (backend, _core, ui) = setup();
    let flow = line(&ui);
    backend.render(flow.id());
    let texts: Vec<String> = backend
        .ops(flow.id())
        .into_iter()
        .filter_map(|op| match op {
            DrawOp::Text(_, text, _) => Some(text),
            _ => None,
        })
        .collect();
    assert_eq!(texts, ["one", " ", "two"]);
}

#[test]
fn clicking_a_link_raises_its_msg() {
    let (_backend, core, ui) = setup();
    let flow = line(&ui);
    let log = Rc::new(RefCell::new(Vec::new()));
    let runtime = Runtime::primary(
        core,
        TestApp {
            log: Rc::clone(&log),
        },
    );

    runtime.deliver(
        flow.id(),
        &Event::MouseDown {
            x: 2,
            y: 4,
            button: MouseButton::Left,
            modifiers: Modifiers::NONE,
        },
    );
    runtime.deliver(
        flow.id(),
        &Event::MouseUp {
            x: 2,
            y: 4,
            button: MouseButton::Left,
            modifiers: Modifiers::NONE,
        },
    );
    runtime.deliver(WidgetId::NONE, &Event::Wake);
    assert_eq!(*log.borrow(), vec![7]);
}

#[test]
fn clicking_a_non_link_raises_nothing() {
    let (_backend, core, ui) = setup();
    let flow = line(&ui);
    let log = Rc::new(RefCell::new(Vec::new()));
    let runtime = Runtime::primary(
        core,
        TestApp {
            log: Rc::clone(&log),
        },
    );

    // The "two" run starts after "one" plus a space, well past x = 30.
    runtime.deliver(
        flow.id(),
        &Event::MouseUp {
            x: 80,
            y: 4,
            button: MouseButton::Left,
            modifiers: Modifiers::NONE,
        },
    );
    runtime.deliver(WidgetId::NONE, &Event::Wake);
    assert!(log.borrow().is_empty());
}

#[test]
fn a_narrower_line_wraps_taller() {
    let (_backend, _core, ui) = setup();
    let flow = FlowText::new(&ui, Rect::new(0, 0, 40, 40))
        .unwrap()
        .run(Run::normal("one two three four"));
    assert!(
        flow.preferred_height(crate::units::dip(40.0)).value() > 20.0,
        "the line wrapped to more than one row"
    );
}

/// The number of distinct rows the painted fragments sit on.
fn painted_rows(backend: &HeadlessBackend, id: WidgetId) -> usize {
    let mut tops: Vec<i32> = backend
        .ops(id)
        .into_iter()
        .filter_map(|op| match op {
            DrawOp::Text(rect, _, _) => Some(rect.top),
            _ => None,
        })
        .collect();
    tops.sort_unstable();
    tops.dedup();
    tops.len()
}

#[test]
fn a_move_that_resizes_the_node_rewraps() {
    let (backend, _core, ui) = setup();
    let flow = FlowText::new(&ui, Rect::new(0, 0, 400, 40))
        .unwrap()
        .run(Run::normal("one two three four five six"));
    backend.render(flow.id());
    assert_eq!(
        painted_rows(&backend, flow.id()),
        1,
        "the wide line is a single row"
    );

    // A container that narrows the node without a backend `Resize` event.
    ui.apply_moves(&[(flow.id(), Rect::new(0, 0, 60, 40))]);
    backend.render(flow.id());
    assert!(
        painted_rows(&backend, flow.id()) > 1,
        "the narrowed line re-wrapped"
    );
}
