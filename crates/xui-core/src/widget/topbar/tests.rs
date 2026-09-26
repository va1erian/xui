use std::cell::RefCell;
use std::rc::Rc;

use super::*;
use crate::app::{App, Core, Runtime};
use crate::backend::headless::HeadlessBackend;
use crate::backend::{Backend, Event, PlatformSpec};
use crate::message::{Modifiers, MouseButton};

struct TestApp(Rc<RefCell<Vec<u32>>>);

impl App for TestApp {
    type Msg = u32;

    fn update(&mut self, msg: u32, _ui: &mut Ui<u32>) {
        self.0.borrow_mut().push(msg);
    }
}

type Harness = (Ui<u32>, Rc<Runtime<TestApp>>, Rc<RefCell<Vec<u32>>>);

fn setup() -> Harness {
    let backend = Rc::new(HeadlessBackend::new());
    let window = backend.open_window(&PlatformSpec::new("test")).unwrap();
    let core = Core::new(backend, window);
    let ui = Ui::new(Rc::clone(&core));
    let log = Rc::new(RefCell::new(Vec::new()));
    let runtime = Runtime::primary(core, TestApp(Rc::clone(&log)));
    (ui, runtime, log)
}

fn click(runtime: &Runtime<TestApp>, target: WidgetId, x: i32) {
    let modifiers = Modifiers::NONE;
    let y = 5;
    for event in [
        Event::MouseDown {
            x,
            y,
            button: MouseButton::Left,
            modifiers,
        },
        Event::MouseMove { x, y, modifiers },
        Event::MouseUp {
            x,
            y,
            button: MouseButton::Left,
            modifiers,
        },
    ] {
        runtime.deliver(target, &event);
    }
    runtime.deliver(WidgetId::NONE, &Event::Wake);
}

#[test]
fn an_icon_click_maps_to_the_apps_message() {
    let (ui, runtime, log) = setup();
    let menu = TopBarId::new(1);
    let bar = TopBar::new(&ui, Rect::new(0, 0, 180, 28))
        .unwrap()
        .icon(menu, Glyph::Menu)
        .on_click(move |id| (id == menu).then_some(7));

    click(&runtime, bar.id(), 10);
    assert_eq!(*log.borrow(), vec![7]);
}

#[test]
fn a_toggle_click_maps_its_new_state() {
    let (ui, runtime, log) = setup();
    let star = TopBarId::new(2);
    let bar = TopBar::new(&ui, Rect::new(0, 0, 180, 28))
        .unwrap()
        .icon(TopBarId::new(1), Glyph::Menu)
        .toggle(star, Glyph::Star)
        .on_toggle(move |id, checked| (id == star).then_some(checked as u32));

    click(&runtime, bar.id(), 50);
    assert!(bar.is_checked(star));
    assert_eq!(*log.borrow(), vec![1]);
}

#[test]
fn dragging_a_slider_maps_its_value() {
    let (ui, runtime, log) = setup();
    let volume = TopBarId::new(3);
    let bar = TopBar::new(&ui, Rect::new(0, 0, 200, 28))
        .unwrap()
        .slider(volume, 0.0, 100.0)
        .on_change(move |id, value| (id == volume).then_some(value.round() as u32));

    click(&runtime, bar.id(), 100);
    assert_eq!(*log.borrow(), vec![87], "the drag raised its value once");
    assert!(bar.value(volume).unwrap() > 80.0);
}

#[test]
fn a_disabled_item_ignores_clicks() {
    let (ui, runtime, log) = setup();
    let menu = TopBarId::new(1);
    let bar = TopBar::new(&ui, Rect::new(0, 0, 180, 28))
        .unwrap()
        .icon(menu, Glyph::Menu)
        .on_click(move |id| (id == menu).then_some(7));
    bar.set_enabled(menu, false);

    click(&runtime, bar.id(), 10);
    assert!(log.borrow().is_empty(), "a disabled item raises nothing");
}

#[test]
fn setters_update_by_id_without_events() {
    let (ui, _runtime, log) = setup();
    let star = TopBarId::new(1);
    let title = TopBarId::new(2);
    let volume = TopBarId::new(3);
    let bar = TopBar::new(&ui, Rect::new(0, 0, 320, 28))
        .unwrap()
        .toggle(star, Glyph::Star)
        .label(title, "old")
        .slider(volume, 0.0, 10.0);

    bar.set_checked(star, true);
    bar.set_text(title, "new");
    bar.set_value(volume, 50.0);
    let bar = bar.tooltip(volume, "Volume");

    assert!(bar.is_checked(star));
    assert_eq!(bar.text(title).as_deref(), Some("new"));
    assert_eq!(
        bar.value(volume),
        Some(10.0),
        "the value clamps to the range"
    );
    assert_eq!(bar.tooltip_text(volume).as_deref(), Some("Volume"));
    assert!(
        log.borrow().is_empty(),
        "programmatic setters raise nothing"
    );
}
