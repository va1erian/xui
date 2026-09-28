use std::cell::RefCell;
use std::rc::Rc;

use super::Tabs;
use crate::app::{App, Core, Runtime, Ui};
use crate::backend::headless::HeadlessBackend;
use crate::backend::{Backend, Event, PlatformSpec, WidgetId};
use crate::geometry::Rect;
use crate::message::{Modifiers, MouseButton};
use crate::property::{Properties, Value};
use crate::widget::Label;

struct TestApp(Rc<RefCell<Vec<usize>>>);

impl App for TestApp {
    type Msg = usize;
    fn update(&mut self, msg: usize, _ui: &mut Ui<usize>) {
        self.0.borrow_mut().push(msg);
    }
}

fn setup() -> (Rc<HeadlessBackend>, Rc<Core<usize>>, Ui<usize>) {
    let backend = Rc::new(HeadlessBackend::new());
    let window = backend.open_window(&PlatformSpec::new("tabs")).unwrap();
    let core = Core::new(backend.clone(), window);
    let ui = Ui::new(Rc::clone(&core));
    (backend, core, ui)
}

fn click(x: i32) -> Event {
    Event::MouseUp {
        x,
        y: 8,
        button: MouseButton::Left,
        modifiers: Modifiers::NONE,
    }
}

#[test]
fn the_selected_page_shows_and_the_others_hide() {
    let (backend, _core, ui) = setup();
    let tabs = Tabs::new(&ui, Rect::new(0, 0, 200, 120)).unwrap();
    let a = Label::new(tabs.ui(), Rect::new(0, 0, 10, 10), "one").unwrap();
    let b = Label::new(tabs.ui(), Rect::new(0, 0, 10, 10), "two").unwrap();
    let tabs = tabs.page("One", &[a.id()]).page("Two", &[b.id()]);

    assert!(backend.node(a.id()).unwrap().3, "the first page is shown");
    assert_eq!(
        backend.node(a.id()).unwrap().1,
        Rect::new(0, 32, 200, 120),
        "its child fills the page area below the strip"
    );
    assert!(
        !backend.node(b.id()).unwrap().3,
        "the second page is hidden"
    );
    assert_eq!(tabs.selected(), 0);
}

#[test]
fn selecting_a_page_swaps_the_visible_children() {
    let (backend, _core, ui) = setup();
    let tabs = Tabs::new(&ui, Rect::new(0, 0, 200, 120)).unwrap();
    let a = Label::new(tabs.ui(), Rect::new(0, 0, 10, 10), "one").unwrap();
    let b = Label::new(tabs.ui(), Rect::new(0, 0, 10, 10), "two").unwrap();
    let tabs = tabs.page("One", &[a.id()]).page("Two", &[b.id()]);
    let core = _core;

    tabs.select(1);
    assert_eq!(tabs.selected(), 1);
    assert!(!backend.node(a.id()).unwrap().3);
    assert!(backend.node(b.id()).unwrap().3);
    assert_eq!(backend.node(b.id()).unwrap().1, Rect::new(0, 32, 200, 120));
    let _ = core;
}

#[test]
fn clicking_a_tab_selects_it_and_raises_the_message() {
    let (_backend, core, ui) = setup();
    let tabs = Tabs::new(&ui, Rect::new(0, 0, 200, 120)).unwrap();
    let a = Label::new(tabs.ui(), Rect::new(0, 0, 10, 10), "one").unwrap();
    let b = Label::new(tabs.ui(), Rect::new(0, 0, 10, 10), "two").unwrap();
    let tabs = tabs
        .page("One", &[a.id()])
        .page("Two", &[b.id()])
        .on_change(Some);
    let log = Rc::new(RefCell::new(Vec::new()));
    let runtime = Runtime::primary(core, TestApp(Rc::clone(&log)));

    let second = tabs.shared.tabs.borrow()[1];
    runtime.deliver(tabs.shared.strip_id, &click(second.left + 2));
    runtime.deliver(WidgetId::NONE, &Event::Wake);
    assert_eq!(tabs.selected(), 1);
    assert_eq!(*log.borrow(), vec![1]);
}

#[test]
fn hovering_a_tab_only_repaints_the_strip() {
    let (backend, _core, ui) = setup();
    let tabs = Tabs::new(&ui, Rect::new(0, 0, 200, 120)).unwrap();
    let a = Label::new(tabs.ui(), Rect::new(0, 0, 10, 10), "one").unwrap();
    let _ = a;
    let tabs = tabs.page("One", &[a.id()]).page("Two", &[]);

    let before = backend.invalidations();
    let strip = tabs.shared.strip_id;
    let runtime = Runtime::primary(_core, TestApp(Rc::new(RefCell::new(Vec::new()))));
    runtime.deliver(
        strip,
        &Event::MouseMove {
            x: 4,
            y: 8,
            modifiers: Modifiers::NONE,
        },
    );
    assert!(backend.invalidations() > before, "hover repaints the strip");
}

#[test]
fn the_selected_property_round_trips() {
    let (_backend, _core, ui) = setup();
    let tabs = Tabs::new(&ui, Rect::new(0, 0, 200, 120)).unwrap();
    let tabs = tabs.page("One", &[]).page("Two", &[]);
    assert_eq!(tabs.property("selected"), Some(Value::Integer(0)));
    assert!(tabs.set_property("selected", Value::Integer(1)));
    assert_eq!(tabs.selected(), 1);
}

#[test]
fn a_tabs_own_design_mode_freezes_its_strip() {
    let (_backend, core, ui) = setup();
    let tabs = Tabs::new(&ui, Rect::new(0, 0, 200, 120)).unwrap();
    let tabs = tabs.page("One", &[]).page("Two", &[]);
    let runtime = Runtime::primary(core, TestApp(Rc::new(RefCell::new(Vec::new()))));
    let second = tabs.shared.tabs.borrow()[1];

    tabs.set_design_mode(true);
    runtime.deliver(tabs.shared.strip_id, &click(second.left + 2));
    assert_eq!(
        tabs.selected(),
        0,
        "the strip ignores clicks in design mode"
    );

    tabs.set_design_mode(false);
    runtime.deliver(tabs.shared.strip_id, &click(second.left + 2));
    assert_eq!(tabs.selected(), 1);
}
