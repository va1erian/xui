#![forbid(unsafe_code)]

//! [`Toolbar`](super::Toolbar) behaviour tests.

use std::cell::RefCell;
use std::rc::Rc;

use super::*;
use crate::app::{App, Core, Runtime};
use crate::backend::headless::HeadlessBackend;
use crate::backend::{Backend, PlatformSpec};
use crate::message::Modifiers;

struct TestApp(Rc<RefCell<Vec<u32>>>);

impl App for TestApp {
    type Msg = u32;

    fn update(&mut self, msg: u32, _ui: &mut Ui<u32>) {
        self.0.borrow_mut().push(msg);
    }
}

fn setup() -> (Rc<HeadlessBackend>, Rc<Core<u32>>, Ui<u32>) {
    let backend = Rc::new(HeadlessBackend::new());
    let window = backend.open_window(&PlatformSpec::new("test")).unwrap();
    let core = Core::new(backend.clone(), window);
    let ui = Ui::new(Rc::clone(&core));
    (backend, core, ui)
}

fn down(x: i32) -> Event {
    Event::MouseDown {
        x,
        y: 5,
        button: MouseButton::Left,
        modifiers: Modifiers::NONE,
    }
}

fn up(x: i32) -> Event {
    Event::MouseUp {
        x,
        y: 5,
        button: MouseButton::Left,
        modifiers: Modifiers::NONE,
    }
}

#[test]
fn clicking_an_item_maps_to_the_apps_message() {
    let (_backend, core, ui) = setup();
    let toolbar = Toolbar::new(&ui, Rect::new(0, 0, 90, 28), &["one", "two", "three"])
        .unwrap()
        .on_click(|index| Some(index as u32));
    let log = Rc::new(RefCell::new(Vec::new()));
    let runtime = Runtime::primary(core, TestApp(Rc::clone(&log)));

    runtime.deliver(toolbar.id(), &down(45));
    runtime.deliver(toolbar.id(), &up(45));
    runtime.deliver(WidgetId::NONE, &Event::Wake);

    assert_eq!(*log.borrow(), vec![1]);
    assert_eq!(toolbar.property("selected"), Some(Value::Integer(1)));
}

#[test]
fn a_click_past_the_last_item_raises_nothing() {
    let (_backend, core, ui) = setup();
    let toolbar = Toolbar::new(&ui, Rect::new(0, 0, 90, 28), &["one", "two", "three"])
        .unwrap()
        .on_click(|index| Some(index as u32));
    let log = Rc::new(RefCell::new(Vec::new()));
    let runtime = Runtime::primary(core, TestApp(Rc::clone(&log)));

    runtime.deliver(toolbar.id(), &down(95));
    runtime.deliver(toolbar.id(), &up(95));
    runtime.deliver(WidgetId::NONE, &Event::Wake);

    assert!(log.borrow().is_empty(), "no item is under x=95");
}

#[test]
fn an_icon_toolbar_builds_lays_out_and_reports_its_items() {
    use crate::icon::Lucide;

    let (backend, _core, ui) = setup();
    let toolbar = Toolbar::empty(&ui, Rect::new(0, 0, 120, 28))
        .unwrap()
        .item(Lucide::Save, "Save")
        .item_with_text(Lucide::Play, "Run", "Run");

    assert_eq!(toolbar.len(), 2);
    assert!(!toolbar.is_empty());
    assert_eq!(toolbar.icon(0), Some(IconRef::Lucide(Lucide::Save)));
    assert_eq!(toolbar.label(0), None);
    assert_eq!(toolbar.tooltip(0).as_deref(), Some("Save"));
    assert_eq!(toolbar.icon(1), Some(IconRef::Lucide(Lucide::Play)));
    assert_eq!(toolbar.label(1).as_deref(), Some("Run"));
    assert_eq!(
        toolbar.tooltip(3),
        None,
        "an index past the end has no item"
    );

    backend.render(toolbar.id());
    assert!(
        !backend.ops(toolbar.id()).is_empty(),
        "the icon toolbar paints its items"
    );
}

#[test]
fn cells_stay_inside_the_strip_and_every_pixel_hits_its_cell() {
    // Wider than the item count with a remainder, and narrower than it.
    for (width, count) in [(10, 3), (3, 5), (100, 7), (1, 1)] {
        let bounds = Rect::new(0, 0, width, 24);
        let mut covered = 0;
        for index in 0..count {
            let (start, end) = cell_span(width, count, index);
            assert!(
                0 <= start && start <= end && end <= width,
                "{width}/{count}: {index}"
            );
            covered += end - start;
            for x in start..end {
                assert_eq!(
                    item_at(bounds, x, count),
                    Some(index),
                    "{width}/{count} at {x}"
                );
            }
        }
        assert_eq!(covered, width, "the cells tile the strip exactly");
        assert_eq!(item_at(bounds, width, count), None, "past the right edge");
        assert_eq!(item_at(bounds, -1, count), None, "before the left edge");
    }
}

#[test]
fn an_empty_or_zero_width_strip_has_no_cells() {
    assert_eq!(cell_span(0, 3, 1), (0, 0));
    assert_eq!(cell_span(10, 0, 0), (0, 0));
    assert_eq!(item_at(Rect::new(0, 0, 0, 24), 0, 3), None);
    assert_eq!(item_at(Rect::new(0, 0, 10, 24), 5, 0), None);
}
