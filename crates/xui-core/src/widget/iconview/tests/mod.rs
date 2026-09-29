#![forbid(unsafe_code)]

//! Unit tests for [`IconView`](super::IconView): pure layout arithmetic,
//! selection modes, pointer, keyboard and context interaction, plus the
//! geometry property tests.

use std::cell::RefCell;
use std::rc::Rc;

use super::layout;
use super::{IconSize, IconView};
use crate::app::{App, Core, Runtime, Ui};
use crate::backend::headless::HeadlessBackend;
use crate::backend::{Backend, Event, PlatformSpec, WidgetId};
use crate::geometry::{Point, Rect};
use crate::message::{Key, Modifiers, MouseButton};

mod interaction;
mod layout_math;
mod properties;
mod selection;

/// A window big enough for several large tiles.
const WIDTH: i32 = 600;
const HEIGHT: i32 = 400;

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

fn log() -> Rc<RefCell<Vec<u32>>> {
    Rc::new(RefCell::new(Vec::new()))
}

/// A view of `count` plain names at the standard test bounds.
fn view(ui: &Ui<u32>, count: usize) -> IconView<u32> {
    let items: Vec<String> = (0..count).map(|index| format!("item {index}")).collect();
    IconView::with_model(ui, Rect::new(0, 0, WIDTH, HEIGHT), items).unwrap()
}

/// The node-local centre of `index`'s tile at the current scroll offset.
fn center(view: &IconView<u32>, index: usize) -> Point {
    let dpi = 96;
    let state = view.state.borrow();
    let metrics = state.metrics(dpi);
    let viewport = state.viewport(Rect::new(0, 0, WIDTH, HEIGHT), dpi);
    let rect = layout::tile_rect(index, viewport.columns, metrics);
    Point::new(
        rect.left + metrics.width / 2,
        rect.top + metrics.height / 2 - state.offset,
    )
}

/// A point that hits no tile: past the last column of the first row.
fn empty_point() -> Point {
    Point::new(WIDTH - 1, 1)
}

fn left(x: i32, y: i32) -> Event {
    Event::MouseDown {
        x,
        y,
        button: MouseButton::Left,
        modifiers: Modifiers::NONE,
    }
}

fn right(x: i32, y: i32) -> Event {
    Event::MouseDown {
        x,
        y,
        button: MouseButton::Right,
        modifiers: Modifiers::NONE,
    }
}

fn left_mod(x: i32, y: i32, modifiers: Modifiers) -> Event {
    Event::MouseDown {
        x,
        y,
        button: MouseButton::Left,
        modifiers,
    }
}

fn double_click(point: Point) -> Event {
    Event::MouseDoubleClick {
        x: point.x,
        y: point.y,
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

fn ctrl() -> Modifiers {
    Modifiers {
        ctrl: true,
        ..Modifiers::NONE
    }
}

fn shift() -> Modifiers {
    Modifiers {
        shift: true,
        ..Modifiers::NONE
    }
}
