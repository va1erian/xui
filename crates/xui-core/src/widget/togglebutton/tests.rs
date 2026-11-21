use std::cell::RefCell;
use std::rc::Rc;

use super::ToggleButton;
use crate::app::{App, Core, Runtime, Ui};
use crate::backend::headless::HeadlessBackend;
use crate::backend::{Backend, Event, PlatformSpec, WidgetId};
use crate::geometry::Rect;
use crate::icon::{IconRef, Lucide};
use crate::message::{Modifiers, MouseButton};
use crate::widget::Glyph;

struct TestApp {
    log: Rc<RefCell<Vec<u32>>>,
}

impl App for TestApp {
    type Msg = u32;
    fn update(&mut self, msg: u32, _ui: &mut Ui<u32>) {
        self.log.borrow_mut().push(msg);
    }
}
type Harness = (Ui<u32>, Rc<Runtime<TestApp>>, Rc<RefCell<Vec<u32>>>);
#[rustfmt::skip]
fn setup() -> Harness {
    let backend = Rc::new(HeadlessBackend::new());
    let window = backend.open_window(&PlatformSpec::new("test")).unwrap();
    let core = Core::new(backend, window);
    let ui = Ui::new(Rc::clone(&core));
    let log = Rc::new(RefCell::new(Vec::new()));
    let runtime = Runtime::primary(core, TestApp { log: Rc::clone(&log) });
    (ui, runtime, log)
}

#[rustfmt::skip]
fn click(runtime: &Runtime<TestApp>, target: WidgetId) {
    let m = Modifiers::NONE;
    let down = Event::MouseDown { x: 5, y: 5, button: MouseButton::Left, modifiers: m };
    let up = Event::MouseUp { x: 5, y: 5, button: MouseButton::Left, modifiers: m };
    let mv = Event::MouseMove { x: 6, y: 5, modifiers: m };
    runtime.deliver(target, &down);
    runtime.deliver(target, &mv);
    runtime.deliver(target, &up);
    runtime.deliver(WidgetId::NONE, &Event::Wake);
}

#[test]
fn a_click_toggles_and_maps_a_message() {
    let (ui, runtime, log) = setup();
    let button = ToggleButton::new(&ui, Rect::new(0, 0, 80, 28), "Bold")
        .unwrap()
        .on_toggle(|checked| Some(checked as u32));
    assert!(!button.is_checked());
    click(&runtime, button.id());
    assert!(button.is_checked());
    assert_eq!(*log.borrow(), vec![1]);
}

#[test]
fn a_disabled_toggle_button_ignores_clicks() {
    let (ui, runtime, log) = setup();
    let button = ToggleButton::new(&ui, Rect::new(0, 0, 80, 28), "Bold")
        .unwrap()
        .on_toggle(|checked| Some(checked as u32));
    button.set_enabled(false);
    click(&runtime, button.id());
    assert!(!button.is_checked(), "a disabled button toggles nothing");
    assert!(log.borrow().is_empty());
}

#[test]
fn set_checked_raises_nothing() {
    let (ui, _runtime, log) = setup();
    let button = ToggleButton::new(&ui, Rect::new(0, 0, 80, 28), "Bold").unwrap();
    button.set_checked(true);
    assert!(button.is_checked());
    assert!(log.borrow().is_empty(), "a programmatic set raised nothing");
}
#[test]
fn icon_is_set_replaced_and_cleared_without_toggling() {
    let (ui, runtime, log) = setup();
    let button = ToggleButton::new(&ui, Rect::new(0, 0, 28, 28), "")
        .unwrap()
        .icon(Lucide::Bold)
        .on_toggle(|checked| Some(checked as u32));
    assert_eq!(button.icon.get(), Some(IconRef::Lucide(Lucide::Bold)));
    button.set_icon(Some(Glyph::Play));
    assert_eq!(button.icon.get(), Some(IconRef::Glyph(Glyph::Play)));
    button.clear_icon();
    assert_eq!(button.icon.get(), None);
    assert!(!button.is_checked(), "changing the icon toggles nothing");
    click(&runtime, button.id());
    assert!(button.is_checked(), "an icon-only button still toggles");
    assert_eq!(*log.borrow(), vec![1]);
}

#[test]
fn hovering_repaints_once_then_stays_quiet() {
    let backend = Rc::new(HeadlessBackend::new());
    let window = backend.open_window(&PlatformSpec::new("test")).unwrap();
    let core = Core::new(backend.clone(), window);
    let ui = Ui::new(Rc::clone(&core));
    let button = ToggleButton::new(&ui, Rect::new(0, 0, 80, 28), "Bold").unwrap();
    let runtime = Runtime::primary(
        core,
        TestApp {
            log: Rc::new(RefCell::new(Vec::new())),
        },
    );
    let mv = |x| Event::MouseMove {
        x,
        y: 5,
        modifiers: Modifiers::NONE,
    };

    let before = backend.invalidations();
    runtime.deliver(button.id(), &mv(4));
    assert_eq!(
        backend.invalidations(),
        before + 1,
        "entering hover repaints"
    );
    for x in 5..25 {
        runtime.deliver(button.id(), &mv(x));
    }
    assert_eq!(
        backend.invalidations(),
        before + 1,
        "moves while hovered repaint nothing"
    );
    runtime.deliver(button.id(), &Event::MouseLeave);
    assert_eq!(backend.invalidations(), before + 2, "leaving repaints");
}
