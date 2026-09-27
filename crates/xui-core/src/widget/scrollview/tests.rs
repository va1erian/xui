use std::cell::RefCell;
use std::rc::Rc;

use super::ScrollView;
use super::scrollbar::thumb_rect;
use crate::app::{App, Core, Runtime, Ui};
use crate::backend::headless::HeadlessBackend;
use crate::backend::{Backend, Event, PlatformSpec, WidgetId};
use crate::geometry::Rect;
use crate::message::{Key, Modifiers, MouseButton};
use crate::property::{Properties, Value};
use crate::units::{Px, dip};
use crate::widget::Label;

struct TestApp(Rc<RefCell<Vec<i32>>>);

impl App for TestApp {
    type Msg = i32;
    fn update(&mut self, msg: i32, _ui: &mut Ui<i32>) {
        self.0.borrow_mut().push(msg);
    }
}

fn setup() -> (Rc<HeadlessBackend>, Rc<Core<i32>>, Ui<i32>) {
    let backend = Rc::new(HeadlessBackend::new());
    let window = backend.open_window(&PlatformSpec::new("scroll")).unwrap();
    let core = Core::new(backend.clone(), window);
    let ui = Ui::new(Rc::clone(&core));
    (backend, core, ui)
}

fn app(core: Rc<Core<i32>>) -> Rc<Runtime<TestApp>> {
    Runtime::primary(core, TestApp(Rc::new(RefCell::new(Vec::new()))))
}

fn down(y: i32) -> Event {
    Event::MouseDown {
        x: 4,
        y,
        button: MouseButton::Left,
        modifiers: Modifiers::NONE,
    }
}

fn at(y: i32) -> Event {
    Event::MouseMove {
        x: 4,
        y,
        modifiers: Modifiers::NONE,
    }
}

#[test]
fn rows_are_stacked_and_culled_as_the_offset_moves() {
    let (backend, _core, ui) = setup();
    let view = ScrollView::new(&ui, Rect::new(0, 0, 100, 80)).unwrap();
    let a = Label::new(view.ui(), Rect::new(0, 0, 10, 10), "a").unwrap();
    let b = Label::new(view.ui(), Rect::new(0, 0, 10, 10), "b").unwrap();
    let c = Label::new(view.ui(), Rect::new(0, 0, 10, 10), "c").unwrap();
    view.add(a.id(), dip(50.0));
    view.add(b.id(), dip(50.0));
    view.add(c.id(), dip(50.0));

    assert_eq!(view.content_height(), Px(150));
    assert_eq!(
        backend.node(a.id()).unwrap().1,
        Rect::new(0, 0, 88, 50),
        "the first row is at the top"
    );
    assert_eq!(
        backend.node(b.id()).unwrap().1,
        Rect::new(0, 50, 88, 100),
        "the second row follows it"
    );

    view.scroll_to(Px(50));
    assert_eq!(view.offset(), Px(50));
    assert!(
        !backend.node(a.id()).unwrap().3,
        "a row scrolled fully out is hidden"
    );
    assert_eq!(
        backend.node(b.id()).unwrap().1,
        Rect::new(0, 0, 88, 50),
        "the second row reaches the top"
    );
}

#[test]
fn a_wheel_and_a_key_scroll_and_raise_the_message() {
    let (_backend, core, ui) = setup();
    let view = ScrollView::new(&ui, Rect::new(0, 0, 100, 80)).unwrap();
    let a = Label::new(view.ui(), Rect::new(0, 0, 10, 10), "a").unwrap();
    view.add(a.id(), dip(200.0));
    let log = Rc::new(RefCell::new(Vec::new()));
    let runtime = Runtime::primary(core, TestApp(Rc::clone(&log)));
    let view = view.on_scroll(|offset| Some(offset.value()));

    runtime.deliver(
        view.shared.id,
        &Event::MouseWheel {
            delta: -120,
            horizontal: false,
            x: 4,
            y: 4,
            modifiers: Modifiers::NONE,
        },
    );
    assert_eq!(view.offset(), Px(48), "a notch scrolls one step down");

    runtime.deliver(
        view.shared.id,
        &Event::KeyDown {
            key: Key::DOWN,
            modifiers: Modifiers::NONE,
            repeat: 1,
            system: false,
        },
    );
    runtime.deliver(WidgetId::NONE, &Event::Wake);
    assert_eq!(view.offset(), Px(72));
    assert_eq!(*log.borrow(), vec![48, 72], "each scroll raised its offset");
}

#[test]
fn dragging_the_thumb_scrolls() {
    let (backend, core, ui) = setup();
    let view = ScrollView::new(&ui, Rect::new(0, 0, 100, 80)).unwrap();
    let a = Label::new(view.ui(), Rect::new(0, 0, 10, 10), "a").unwrap();
    view.add(a.id(), dip(800.0));
    let runtime = app(core);

    let track = view.shared.bar.track();
    let thumb = thumb_rect(track, view.shared.metrics(), ui.dpi()).expect("a thumb");
    let middle = thumb.top + thumb.height() / 2;
    runtime.deliver(view.shared.bar.id(), &down(middle));
    assert_eq!(
        backend.captured(),
        Some(view.shared.bar.id()),
        "the thumb drag captures the pointer"
    );
    runtime.deliver(view.shared.bar.id(), &at(middle + 40));
    assert!(
        view.offset().value() > 0,
        "dragging the thumb down scrolls down"
    );
    runtime.deliver(
        view.shared.bar.id(),
        &Event::MouseUp {
            x: 4,
            y: middle + 40,
            button: MouseButton::Left,
            modifiers: Modifiers::NONE,
        },
    );
    assert_eq!(backend.captured(), None, "the release drops the capture");
    let settled = view.offset();
    runtime.deliver(view.shared.bar.id(), &at(middle + 80));
    assert_eq!(view.offset(), settled, "a released thumb stops following");
}

#[test]
fn the_view_clips_its_content_to_its_bounds() {
    let (backend, _core, ui) = setup();
    let view = ScrollView::new(&ui, Rect::new(0, 0, 100, 80)).unwrap();
    let a = Label::new(view.ui(), Rect::new(0, 0, 10, 10), "a").unwrap();
    view.add(a.id(), dip(200.0));

    assert_eq!(
        backend.clip(view.id()),
        Some(Rect::new(0, 0, 100, 80)),
        "the view clips its descendants to its own bounds"
    );
}

#[test]
fn content_that_fits_has_no_scrollbar() {
    let (backend, _core, ui) = setup();
    let view = ScrollView::new(&ui, Rect::new(0, 0, 100, 200)).unwrap();
    let a = Label::new(view.ui(), Rect::new(0, 0, 10, 10), "a").unwrap();
    view.add(a.id(), dip(50.0));
    assert!(
        !backend.node(view.shared.bar.id()).unwrap().3,
        "the bar is hidden"
    );
    assert_eq!(view.offset(), Px(0));
}

#[test]
fn the_offset_property_round_trips() {
    let (_backend, _core, ui) = setup();
    let view = ScrollView::new(&ui, Rect::new(0, 0, 100, 80)).unwrap();
    let a = Label::new(view.ui(), Rect::new(0, 0, 10, 10), "a").unwrap();
    view.add(a.id(), dip(200.0));
    assert_eq!(view.property("offset"), Some(Value::Integer(0)));
    assert!(view.set_property("offset", Value::Integer(30)));
    assert_eq!(view.offset(), Px(30));
}
