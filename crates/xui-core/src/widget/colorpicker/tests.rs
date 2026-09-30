use std::cell::RefCell;
use std::rc::Rc;

use super::ColorPicker;
use crate::app::{App, Core, Runtime, Ui};
use crate::backend::headless::{DrawOp, HeadlessBackend};
use crate::backend::{Backend, Event, PlatformSpec, WidgetId};
use crate::geometry::{Point, Rect};
use crate::message::{Key, Modifiers, MouseButton};

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
    let window = backend.open_window(&PlatformSpec::new("picker")).unwrap();
    let core = Core::new(backend.clone(), window);
    let ui = Ui::new(Rc::clone(&core));
    (backend, core, ui)
}

fn colors() -> [crate::Color; 3] {
    [
        crate::Color::rgb(10, 0, 0),
        crate::Color::rgb(20, 0, 0),
        crate::Color::rgb(30, 0, 0),
    ]
}

fn click(x: i32, y: i32) -> Event {
    Event::MouseDown {
        x,
        y,
        button: MouseButton::Left,
        modifiers: Modifiers::NONE,
    }
}

fn key(key: Key) -> Event {
    Event::KeyDown {
        key,
        modifiers: Modifiers::NONE,
        repeat: 1,
        system: false,
    }
}

fn runtime(core: Rc<Core<u32>>, log: &Rc<RefCell<Vec<u32>>>) -> Rc<Runtime<TestApp>> {
    Runtime::primary(
        core,
        TestApp {
            log: Rc::clone(log),
        },
    )
}

#[test]
fn clicking_a_swatch_selects_it_and_maps_the_colour() {
    let (_backend, core, ui) = setup();
    let palette = colors();
    let picker = ColorPicker::new(&ui, Rect::new(0, 0, 90, 30), &palette)
        .unwrap()
        .columns(3)
        .on_select(|color| Some(color.r as u32));
    let log = Rc::new(RefCell::new(Vec::new()));
    let runtime = runtime(core, &log);

    runtime.deliver(picker.id(), &click(10, 10));
    runtime.deliver(WidgetId::NONE, &Event::Wake);
    assert_eq!(picker.color(), Some(palette[0]));

    runtime.deliver(picker.id(), &click(70, 10));
    runtime.deliver(WidgetId::NONE, &Event::Wake);
    assert_eq!(
        picker.color(),
        Some(palette[2]),
        "the third swatch was picked"
    );
    assert_eq!(*log.borrow(), vec![10, 30]);
}

#[test]
fn a_picker_away_from_the_origin_hit_tests_in_its_own_coordinates() {
    let (_backend, core, ui) = setup();
    let palette = colors();
    // Bounds are parent-relative; pointer events are node-local.
    let picker = ColorPicker::new(&ui, Rect::new(40, 30, 130, 60), &palette)
        .unwrap()
        .columns(3)
        .on_select(|color| Some(color.r as u32));
    let log = Rc::new(RefCell::new(Vec::new()));
    let runtime = runtime(core, &log);

    runtime.deliver(picker.id(), &click(10, 10));
    runtime.deliver(WidgetId::NONE, &Event::Wake);
    assert_eq!(picker.color(), Some(palette[0]));

    runtime.deliver(picker.id(), &click(70, 10));
    runtime.deliver(WidgetId::NONE, &Event::Wake);
    assert_eq!(picker.color(), Some(palette[2]));

    // Outside the grid's own extent (it is 30 px tall) nothing is picked.
    runtime.deliver(picker.id(), &click(10, 45));
    runtime.deliver(WidgetId::NONE, &Event::Wake);
    assert_eq!(*log.borrow(), vec![10, 30]);
}

#[test]
fn the_keyboard_moves_the_selection() {
    let (_backend, core, ui) = setup();
    let palette = colors();
    let picker = ColorPicker::new(&ui, Rect::new(0, 0, 90, 30), &palette)
        .unwrap()
        .columns(3)
        .selected(palette[0])
        .on_select(|color| Some(color.r as u32));
    let log = Rc::new(RefCell::new(Vec::new()));
    let runtime = runtime(core, &log);

    runtime.deliver(picker.id(), &key(Key::RIGHT));
    runtime.deliver(picker.id(), &key(Key::END));
    runtime.deliver(WidgetId::NONE, &Event::Wake);

    assert_eq!(picker.color(), Some(palette[2]));
    assert_eq!(*log.borrow(), vec![20, 30]);
}

#[test]
fn selecting_from_code_raises_nothing() {
    let (_backend, core, ui) = setup();
    let palette = colors();
    let picker = ColorPicker::new(&ui, Rect::new(0, 0, 90, 30), &palette)
        .unwrap()
        .columns(3)
        .on_select(|color| Some(color.r as u32));
    let log = Rc::new(RefCell::new(Vec::new()));
    let _runtime = runtime(core, &log);

    picker.select(palette[1]);
    assert_eq!(picker.color(), Some(palette[1]));
    assert!(
        log.borrow().is_empty(),
        "a programmatic select raised nothing"
    );
}

#[test]
fn every_swatch_is_painted() {
    let (backend, _core, ui) = setup();
    let palette = colors();
    let picker = ColorPicker::new(&ui, Rect::new(0, 0, 90, 30), &palette)
        .unwrap()
        .columns(3);

    backend.render(picker.id());
    let ops = backend.ops(picker.id());
    for color in palette {
        assert!(
            ops.iter()
                .any(|op| matches!(op, DrawOp::Rounded(_, _, painted) if *painted == color)),
            "the swatch {color:?} was painted: {ops:?}"
        );
    }
}

#[test]
fn the_selected_check_is_not_stretched_on_a_wide_swatch() {
    let (backend, _core, ui) = setup();
    let palette = [crate::Color::rgb(10, 20, 30)];
    let picker = ColorPicker::new(&ui, Rect::new(0, 0, 200, 40), &palette)
        .unwrap()
        .columns(1)
        .selected(palette[0]);

    backend.render(picker.id());
    let lines: Vec<(Point, Point)> = backend
        .ops(picker.id())
        .into_iter()
        .filter_map(|op| match op {
            DrawOp::Line(a, b, _, _) => Some((a, b)),
            _ => None,
        })
        .collect();
    assert_eq!(lines.len(), 2, "the check mark is two line segments");
    let xs: Vec<i32> = lines.iter().flat_map(|(a, b)| [a.x, b.x]).collect();
    let ys: Vec<i32> = lines.iter().flat_map(|(a, b)| [a.y, b.y]).collect();
    let span_x = xs.iter().max().unwrap() - xs.iter().min().unwrap();
    let span_y = ys.iter().max().unwrap() - ys.iter().min().unwrap();
    assert!(
        span_x <= span_y + 4,
        "the check stretched to {span_x}x{span_y} on the wide swatch"
    );
}
