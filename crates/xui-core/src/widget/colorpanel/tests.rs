#![forbid(unsafe_code)]

//! Headless event tests for [`ColorPanel`](super::ColorPanel),
//! [`ColorField`](super::field::ColorField) and
//! [`HueSlider`](super::hue::HueSlider).

mod model_tests;

use std::cell::RefCell;
use std::rc::Rc;

use super::ColorPanel;
use super::field::ColorField;
use super::hue::HueSlider;
use super::model::{BASIC_COLORS, Hsv};
use super::text::Field;
use crate::Color;
use crate::app::{App, Core, Runtime, Ui};
use crate::backend::headless::HeadlessBackend;
use crate::backend::{Backend, Event, PlatformSpec, WidgetId};
use crate::geometry::Rect;
use crate::message::{Key, Modifiers, MouseButton};
use crate::widget::HasText;

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
    let window = backend.open_window(&PlatformSpec::new("colour")).unwrap();
    let core = Core::new(backend.clone(), window);
    let ui = Ui::new(Rc::clone(&core));
    (backend, core, ui)
}

fn runtime(core: Rc<Core<u32>>, log: &Rc<RefCell<Vec<u32>>>) -> Rc<Runtime<TestApp>> {
    Runtime::primary(
        core,
        TestApp {
            log: Rc::clone(log),
        },
    )
}

fn down(x: i32, y: i32) -> Event {
    Event::MouseDown {
        x,
        y,
        button: MouseButton::Left,
        modifiers: Modifiers::NONE,
    }
}

fn up(x: i32, y: i32) -> Event {
    Event::MouseUp {
        x,
        y,
        button: MouseButton::Left,
        modifiers: Modifiers::NONE,
    }
}

fn moved(x: i32, y: i32) -> Event {
    Event::MouseMove {
        x,
        y,
        modifiers: Modifiers::NONE,
    }
}

fn key(key: Key, shift: bool) -> Event {
    Event::KeyDown {
        key,
        modifiers: Modifiers {
            shift,
            ..Modifiers::NONE
        },
        repeat: 1,
        system: false,
    }
}

fn drain(runtime: &Runtime<TestApp>) {
    runtime.deliver(WidgetId::NONE, &Event::Wake);
}

#[test]
fn the_sv_field_captures_on_press_and_commits_once_on_release() {
    let (backend, core, ui) = setup();
    let field = ColorField::new(&ui, Rect::new(0, 0, 200, 100), Hsv::new(0.0, 1.0, 1.0))
        .unwrap()
        .on_change(|_| Some(1))
        .on_commit(|_| Some(2));
    let log = Rc::new(RefCell::new(Vec::new()));
    let runtime = runtime(core, &log);

    runtime.deliver(field.id(), &down(20, 20));
    assert_eq!(backend.captured(), Some(field.id()));
    runtime.deliver(field.id(), &moved(60, 40));
    runtime.deliver(field.id(), &moved(80, 50));
    runtime.deliver(field.id(), &up(80, 50));
    assert_eq!(backend.captured(), None);
    drain(&runtime);

    let commits = log.borrow().iter().filter(|msg| **msg == 2).count();
    assert_eq!(commits, 1, "exactly one commit: {:?}", log.borrow());
    assert!(log.borrow().contains(&1), "drag ticks changed");
}

#[test]
fn a_captured_drag_survives_mouse_leave_and_commits_when_capture_is_lost() {
    let (backend, core, ui) = setup();
    let field = ColorField::new(&ui, Rect::new(0, 0, 200, 100), Hsv::new(0.0, 1.0, 1.0))
        .unwrap()
        .on_change(|_| Some(1))
        .on_commit(|_| Some(2));
    let slider = HueSlider::new(&ui, Rect::new(0, 200, 360, 220), 0.0)
        .unwrap()
        .on_change(|_| Some(3))
        .on_commit(|_| Some(4));
    let log = Rc::new(RefCell::new(Vec::new()));
    let runtime = runtime(core, &log);

    runtime.deliver(field.id(), &down(20, 20));
    runtime.deliver(field.id(), &Event::MouseLeave);
    assert_eq!(
        backend.captured(),
        Some(field.id()),
        "leave keeps the capture"
    );
    runtime.deliver(field.id(), &moved(60, 40));
    runtime.deliver(field.id(), &Event::CaptureChanged);
    runtime.deliver(field.id(), &moved(90, 60));
    drain(&runtime);
    assert_eq!(log.borrow().iter().filter(|m| **m == 2).count(), 1);
    assert_eq!(log.borrow().iter().filter(|m| **m == 1).count(), 2);

    log.borrow_mut().clear();
    runtime.deliver(slider.id(), &down(90, 210));
    runtime.deliver(slider.id(), &Event::MouseLeave);
    runtime.deliver(slider.id(), &moved(180, 210));
    runtime.deliver(slider.id(), &Event::CaptureChanged);
    drain(&runtime);
    assert!(
        (slider.hue() - 180.0).abs() < 1.0,
        "drag continued after leave"
    );
    assert_eq!(log.borrow().iter().filter(|m| **m == 4).count(), 1);
}

#[test]
fn the_sv_field_clamps_a_pointer_dragged_outside() {
    let (_backend, core, ui) = setup();
    let field = ColorField::new(&ui, Rect::new(0, 0, 200, 100), Hsv::new(30.0, 0.5, 0.5)).unwrap();
    let log = Rc::new(RefCell::new(Vec::new()));
    let runtime = runtime(core, &log);

    runtime.deliver(field.id(), &down(1000, 1000));
    let hsv = field.hsv();
    assert_eq!((hsv.s, hsv.v), (1.0, 0.0));
    assert_eq!(hsv.h, 30.0, "the hue is preserved");
    runtime.deliver(field.id(), &up(1000, 1000));

    runtime.deliver(field.id(), &down(-40, -40));
    assert_eq!((field.hsv().s, field.hsv().v), (0.0, 1.0));
}

#[test]
fn the_sv_field_nudges_with_the_keyboard() {
    let (_backend, core, ui) = setup();
    let field = ColorField::new(&ui, Rect::new(0, 0, 200, 100), Hsv::new(200.0, 0.5, 0.5))
        .unwrap()
        .on_commit(|_| Some(1));
    let log = Rc::new(RefCell::new(Vec::new()));
    let runtime = runtime(core, &log);

    runtime.deliver(field.id(), &key(Key::RIGHT, false));
    assert_eq!(field.hsv().s, 0.51);
    runtime.deliver(field.id(), &key(Key::UP, true));
    assert_eq!(field.hsv().v, 0.6);
    // A downward nudge at zero value keeps the hue.
    field.set_hsv(Hsv::new(200.0, 0.5, 0.0));
    runtime.deliver(field.id(), &key(Key::DOWN, false));
    assert_eq!(field.hsv().h, 200.0);
    assert_eq!(field.hsv().v, 0.0);
    drain(&runtime);
}

#[test]
fn the_hue_slider_drags_and_steps() {
    let (backend, core, ui) = setup();
    let slider = HueSlider::new(&ui, Rect::new(0, 0, 360, 20), 0.0)
        .unwrap()
        .on_commit(|_| Some(7));
    let log = Rc::new(RefCell::new(Vec::new()));
    let runtime = runtime(core, &log);

    runtime.deliver(slider.id(), &down(180, 10));
    assert_eq!(backend.captured(), Some(slider.id()));
    assert!((slider.hue() - 180.0).abs() < 1.0);
    runtime.deliver(slider.id(), &up(180, 10));
    assert_eq!(backend.captured(), None);

    slider.set_hue(0.0);
    runtime.deliver(slider.id(), &key(Key::RIGHT, false));
    assert_eq!(slider.hue(), 1.0);
    runtime.deliver(slider.id(), &key(Key::RIGHT, true));
    assert_eq!(slider.hue(), 11.0);
    runtime.deliver(slider.id(), &key(Key::END, false));
    assert_eq!(slider.hue(), 360.0);
    runtime.deliver(slider.id(), &key(Key::HOME, false));
    assert_eq!(slider.hue(), 0.0);
    drain(&runtime);
}

#[test]
fn set_color_updates_every_view_without_firing_on_change() {
    let (_backend, core, ui) = setup();
    let panel = ColorPanel::new(&ui, Rect::new(0, 0, 640, 420))
        .unwrap()
        .on_change(|_| Some(1))
        .on_commit(|_| Some(2));
    let log = Rc::new(RefCell::new(Vec::new()));
    let runtime = runtime(core, &log);

    panel.set_color(Color::hex(0xEB_40_34));
    drain(&runtime);
    assert!(log.borrow().is_empty(), "a programmatic set is silent");
    assert_eq!(panel.color(), Color::hex(0xEB_40_34));
    assert!((panel.field_hsv().h - 4.0).abs() < 0.5);
    assert_eq!(panel.hue_value().round(), 4.0);
    assert_eq!(panel.edit_handle(Field::Hex).unwrap().text(), "#eb4034");
    assert_eq!(panel.edit_handle(Field::Rgb).unwrap().text(), "235, 64, 52");
    assert_eq!(
        panel.edit_handle(Field::Cmyk).unwrap().text(),
        "0%, 73%, 78%, 8%"
    );
    assert_eq!(
        panel.edit_handle(Field::Hsv).unwrap().text(),
        "4°, 78%, 92%"
    );
    assert_eq!(
        panel.edit_handle(Field::Hsl).unwrap().text(),
        "4°, 82%, 56%"
    );
}

#[test]
fn a_live_valid_edit_updates_the_other_views_but_not_its_own_box() {
    let (_backend, core, ui) = setup();
    let panel = ColorPanel::new(&ui, Rect::new(0, 0, 640, 420))
        .unwrap()
        .on_change(|_| Some(1));
    let log = Rc::new(RefCell::new(Vec::new()));
    let runtime = runtime(core, &log);

    let rgb = panel.edit_handle(Field::Rgb).unwrap();
    // A raw, valid spelling: it must not be overwritten mid-edit.
    rgb.set_text("0200, 010, 020");
    runtime.deliver(rgb.id(), &Event::TextChanged);
    drain(&runtime);

    assert_eq!(panel.color(), Color::rgb(200, 10, 20));
    assert_eq!(rgb.text(), "0200, 010, 020", "the edited box is left alone");
    assert_eq!(
        panel.edit_handle(Field::Hex).unwrap().text(),
        "#c80a14",
        "the other boxes refreshed"
    );
    assert_eq!(*log.borrow(), vec![1]);
}

#[test]
fn invalid_text_reverts_without_changing_the_colour() {
    let (_backend, core, ui) = setup();
    let panel = ColorPanel::new(&ui, Rect::new(0, 0, 640, 420)).unwrap();
    let log = Rc::new(RefCell::new(Vec::new()));
    let runtime = runtime(core, &log);
    panel.set_color(Color::hex(0xEB_40_34));

    let edit = panel.edit_handle(Field::Rgb).unwrap();
    edit.set_text("300, 0, 0");
    runtime.deliver(edit.id(), &Event::KillFocus);
    drain(&runtime);
    assert_eq!(panel.color(), Color::hex(0xEB_40_34));
    assert_eq!(edit.text(), "235, 64, 52", "the box reverted");
    assert!(log.borrow().is_empty(), "an invalid commit raises nothing");
}

#[test]
fn valid_text_commits_on_focus_loss() {
    let (_backend, core, ui) = setup();
    let panel = ColorPanel::new(&ui, Rect::new(0, 0, 640, 420))
        .unwrap()
        .on_commit(|color| Some(u32::from(color.r)));
    let log = Rc::new(RefCell::new(Vec::new()));
    let runtime = runtime(core, &log);

    let edit = panel.edit_handle(Field::Rgb).unwrap();
    edit.set_text("10, 20, 30");
    runtime.deliver(edit.id(), &Event::KillFocus);
    drain(&runtime);
    assert_eq!(panel.color(), Color::rgb(10, 20, 30));
    assert_eq!(*log.borrow(), vec![10]);
}

#[test]
fn switching_tabs_ends_an_active_drag() {
    let (backend, core, ui) = setup();
    let panel = ColorPanel::new(&ui, Rect::new(0, 0, 640, 420)).unwrap();
    let log = Rc::new(RefCell::new(Vec::new()));
    let runtime = runtime(core, &log);

    runtime.deliver(panel.field_node(), &down(10, 10));
    assert_eq!(backend.captured(), Some(panel.field_node()));
    panel.select_tab(1);
    assert_eq!(panel.tab(), 1);
    assert_eq!(backend.captured(), None, "the tab switch released the drag");
}

#[test]
fn two_panels_keep_their_own_state_and_route_their_own_messages() {
    let (_backend, core, ui) = setup();
    let first = ColorPanel::new(&ui, Rect::new(0, 0, 300, 300))
        .unwrap()
        .on_change(|_| Some(1));
    let second = ColorPanel::new(&ui, Rect::new(320, 0, 620, 300))
        .unwrap()
        .on_change(|_| Some(2));
    let log = Rc::new(RefCell::new(Vec::new()));
    let runtime = runtime(core, &log);

    runtime.deliver(first.field_node(), &down(50, 50));
    drain(&runtime);
    assert_eq!(*log.borrow(), vec![1]);
    assert_eq!(
        second.color(),
        Color::rgb(0, 0, 0),
        "the other is untouched"
    );

    first.set_color(Color::rgb(1, 2, 3));
    assert_eq!(first.color(), Color::rgb(1, 2, 3));
    assert_eq!(second.color(), Color::rgb(0, 0, 0));
}

#[test]
fn a_zero_sized_panel_and_field_do_not_panic() {
    let (_backend, core, ui) = setup();
    let panel = ColorPanel::new(&ui, Rect::default()).unwrap();
    crate::widget::Placeable::placed(&panel, &ui, Rect::default());
    let field = ColorField::new(&ui, Rect::default(), Hsv::new(0.0, 0.5, 0.5)).unwrap();
    let log = Rc::new(RefCell::new(Vec::new()));
    let runtime = runtime(core, &log);
    runtime.deliver(field.id(), &down(0, 0));
    runtime.deliver(field.id(), &moved(0, 0));
    runtime.deliver(field.id(), &up(0, 0));
    let _ = panel;
}

#[test]
fn a_simple_swatch_pick_changes_and_commits_the_colour() {
    let (backend, core, ui) = setup();
    let panel = ColorPanel::new(&ui, Rect::new(0, 0, 640, 420))
        .unwrap()
        .on_change(|_| Some(1))
        .on_commit(|color| Some(u32::from(color.r) + 100));
    let log = Rc::new(RefCell::new(Vec::new()));
    let runtime = runtime(core, &log);

    let (_, bounds, ..) = backend.node(panel.swatch_node()).expect("a swatch grid");
    let x = bounds.width() / 16;
    let y = bounds.height() / 8;
    runtime.deliver(panel.swatch_node(), &down(x, y));
    drain(&runtime);
    assert_eq!(panel.color(), BASIC_COLORS[0]);
    assert_eq!(*log.borrow(), vec![1, 100]);
}

#[test]
fn dropping_a_panel_destroys_its_nodes() {
    let (backend, _core, ui) = setup();
    let panel = ColorPanel::new(&ui, Rect::new(0, 0, 640, 420)).unwrap();
    let root = panel.id();
    let field = panel.field_node();
    assert!(backend.has_node(root));
    assert!(backend.has_node(field));
    drop(panel);
    assert!(!backend.has_node(root), "the root is gone");
    assert!(!backend.has_node(field), "the children are gone with it");
}
