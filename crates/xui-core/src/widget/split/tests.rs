use std::cell::RefCell;
use std::rc::Rc;

use super::Split;
use crate::app::{App, Core, Runtime, Ui};
use crate::backend::headless::HeadlessBackend;
use crate::backend::{Backend, Event, PlatformSpec, WidgetId};
use crate::geometry::Rect;
use crate::message::{Key, Modifiers, MouseButton};
use crate::property::{Properties, Value};
use crate::units::dip;
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
    let window = backend.open_window(&PlatformSpec::new("split")).unwrap();
    let core = Core::new(backend.clone(), window);
    let ui = Ui::new(Rc::clone(&core));
    (backend, core, ui)
}

fn mouse_down(x: i32) -> Event {
    Event::MouseDown {
        x,
        y: 50,
        button: MouseButton::Left,
        modifiers: Modifiers::NONE,
    }
}

fn mouse_move(x: i32) -> Event {
    Event::MouseMove {
        x,
        y: 50,
        modifiers: Modifiers::NONE,
    }
}

/// The divider-local coordinate (x for a row, y for a column) that puts the
/// pointer at window `axis`, as a backend computes it from the divider's
/// current bounds. The splits below sit at the window origin.
fn local_axis(backend: &HeadlessBackend, divider: WidgetId, axis: i32, horizontal: bool) -> i32 {
    let bounds = backend.node(divider).unwrap().1;
    if horizontal {
        axis - bounds.left
    } else {
        axis - bounds.top
    }
}

#[test]
fn the_divider_splits_the_panes_at_the_position() {
    let (backend, _core, ui) = setup();
    let split = Split::row(&ui, Rect::new(0, 0, 300, 100)).unwrap();
    let a = Label::new(split.ui(), Rect::new(0, 0, 10, 10), "a").unwrap();
    let b = Label::new(split.ui(), Rect::new(0, 0, 10, 10), "b").unwrap();
    split.pane_a(&[a.id()]);
    split.pane_b(&[b.id()]);
    split.set_min(dip(40.0), dip(40.0));
    split.set_position(dip(100.0));

    assert_eq!(backend.node(a.id()).unwrap().1, Rect::new(0, 0, 100, 100));
    assert_eq!(
        split.shared.divider.get(),
        Rect::new(100, 0, 105, 100),
        "the divider sits between the panes"
    );
    assert_eq!(backend.node(b.id()).unwrap().1, Rect::new(105, 0, 300, 100));
}

#[test]
fn the_panes_clamp_to_their_minimums() {
    let (_backend, _core, ui) = setup();
    let split = Split::row(&ui, Rect::new(0, 0, 300, 100)).unwrap();
    split.set_min(dip(120.0), dip(120.0));
    split.set_position(dip(10.0));
    assert!(
        (split.position().value() - 120.0).abs() < 0.01,
        "clamped up"
    );
    split.set_position(dip(290.0));
    assert!(
        (split.position().value() - 175.0).abs() < 0.01,
        "clamped to leave the second pane its minimum"
    );
}

#[test]
fn dragging_the_divider_resizes_and_raises_the_message() {
    let (backend, core, ui) = setup();
    let split = Split::row(&ui, Rect::new(0, 0, 300, 100)).unwrap();
    let a = Label::new(split.ui(), Rect::new(0, 0, 10, 10), "a").unwrap();
    split.pane_a(&[a.id()]);
    split.set_position(dip(100.0));
    let log = Rc::new(RefCell::new(Vec::new()));
    let runtime = Runtime::primary(core, TestApp(Rc::clone(&log)));
    let split = split.on_moved(|position| Some(position.value() as i32));

    runtime.deliver(split.shared.divider_id, &mouse_down(2));
    assert_eq!(
        backend.captured(),
        Some(split.shared.divider_id),
        "the divider drag captures the pointer"
    );
    runtime.deliver(split.shared.divider_id, &mouse_move(102));
    runtime.deliver(WidgetId::NONE, &Event::Wake);
    assert!(
        (split.position().value() - 200.0).abs() < 0.01,
        "the drag moved the divider with the pointer"
    );
    assert_eq!(*log.borrow(), vec![200]);

    runtime.deliver(
        split.shared.divider_id,
        &Event::MouseUp {
            x: 102,
            y: 50,
            button: MouseButton::Left,
            modifiers: Modifiers::NONE,
        },
    );
    assert_eq!(backend.captured(), None, "the release drops the capture");
    runtime.deliver(split.shared.divider_id, &mouse_move(50));
    assert!(
        (split.position().value() - 200.0).abs() < 0.01,
        "a released divider stops following"
    );
}

#[test]
fn dragging_tracks_the_cursor_one_to_one_without_oscillating() {
    let (backend, core, ui) = setup();
    let split = Split::row(&ui, Rect::new(0, 0, 300, 100)).unwrap();
    split.set_position(dip(100.0));
    let runtime = Runtime::primary(core, TestApp(Rc::new(RefCell::new(Vec::new()))));
    let divider = split.shared.divider_id;
    let grab = 2;

    runtime.deliver(divider, &mouse_down(grab));
    // The divider moves under the cursor between events, so the local point
    // must be recomputed from its new bounds, as a backend does.
    for cursor in [150, 200, 260, 120, 40, 294, 10] {
        let local = local_axis(&backend, divider, cursor, true);
        runtime.deliver(divider, &mouse_move(local));
        // The panes span 300px with a 5px divider, so the extent ranges 0..295.
        let expected = (cursor - grab).clamp(0, 295);
        assert!(
            (split.position().value() - expected as f32).abs() < 0.01,
            "cursor {cursor} should put the divider at {expected}, got {}",
            split.position().value()
        );
    }
}

#[test]
fn dragging_respects_the_clamps_and_tracks_again_after_them() {
    let (backend, core, ui) = setup();
    let split = Split::row(&ui, Rect::new(0, 0, 300, 100)).unwrap();
    split.set_min(dip(40.0), dip(40.0));
    split.set_position(dip(100.0));
    let runtime = Runtime::primary(core, TestApp(Rc::new(RefCell::new(Vec::new()))));
    let divider = split.shared.divider_id;

    runtime.deliver(divider, &mouse_down(2));
    // Past the maximum: 295 available minus pane B's 40 leaves 255.
    let local = local_axis(&backend, divider, 290, true);
    runtime.deliver(divider, &mouse_move(local));
    assert!(
        (split.position().value() - 255.0).abs() < 0.01,
        "the drag clamps to the maximum"
    );
    // Back before the start: target 100 + (10 - 102) = 8, clamped to 40.
    let local = local_axis(&backend, divider, 10, true);
    runtime.deliver(divider, &mouse_move(local));
    assert!(
        (split.position().value() - 40.0).abs() < 0.01,
        "the drag clamps to the minimum"
    );
    // Back in the middle it tracks the cursor again, not the clamped edge.
    let local = local_axis(&backend, divider, 150, true);
    runtime.deliver(divider, &mouse_move(local));
    assert!(
        (split.position().value() - 148.0).abs() < 0.01,
        "the drag follows the cursor after leaving a clamp"
    );
}

#[test]
fn dragging_a_column_tracks_the_vertical_axis() {
    let (backend, core, ui) = setup();
    let split = Split::column(&ui, Rect::new(0, 0, 100, 300)).unwrap();
    split.set_position(dip(100.0));
    let runtime = Runtime::primary(core, TestApp(Rc::new(RefCell::new(Vec::new()))));
    let divider = split.shared.divider_id;

    runtime.deliver(
        divider,
        &Event::MouseDown {
            x: 50,
            y: 2,
            button: MouseButton::Left,
            modifiers: Modifiers::NONE,
        },
    );
    let local = local_axis(&backend, divider, 200, false);
    runtime.deliver(
        divider,
        &Event::MouseMove {
            x: 50,
            y: local,
            modifiers: Modifiers::NONE,
        },
    );
    assert!(
        (split.position().value() - 198.0).abs() < 0.01,
        "a vertical drag follows the pointer on y"
    );
}

#[test]
fn the_arrow_keys_nudge_the_divider() {
    let (_backend, core, ui) = setup();
    let split = Split::row(&ui, Rect::new(0, 0, 300, 100)).unwrap();
    split.set_position(dip(100.0));
    let runtime = Runtime::primary(core, TestApp(Rc::new(RefCell::new(Vec::new()))));

    runtime.deliver(
        split.shared.divider_id,
        &Event::KeyDown {
            key: Key::RIGHT,
            modifiers: Modifiers::NONE,
            repeat: 1,
            system: false,
        },
    );
    assert!((split.position().value() - 108.0).abs() < 0.01);
}

#[test]
fn the_position_property_round_trips() {
    let (_backend, _core, ui) = setup();
    let split = Split::column(&ui, Rect::new(0, 0, 100, 300)).unwrap();
    assert!(split.set_property("position", Value::Float(80.0)));
    assert!((split.position().value() - 80.0).abs() < 0.01);
    assert_eq!(split.property("position"), Some(Value::Float(80.0)));
}
