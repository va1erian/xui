#![forbid(unsafe_code)]

//! Unit tests for [`ListView`](super::ListView): virtualization, columns,
//! selection modes, the sort and context hooks and keyboard navigation.

use std::cell::RefCell;
use std::rc::Rc;

use super::state::ROW;
use super::{CellData, ListModel, ListView, SelectionMode};

// Re-exported so the tests can keep writing `super::Fill` and friends now that
// they live in child modules of this `tests` module.
pub use super::{ColumnWidth, Fill, SortDirection};
use crate::app::{App, Core, Runtime, Ui};
use crate::backend::headless::{DrawOp, HeadlessBackend};
use crate::backend::{Backend, Event, PlatformSpec, WidgetId};
use crate::geometry::{Point, Rect};
use crate::message::{Key, Modifiers, MouseButton};
use crate::units::Dip;

mod context;
mod header;
mod keyboard;
mod placement;
mod rendering;
mod selection;

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

fn down(x: i32, y: i32) -> Event {
    Event::MouseDown {
        x,
        y,
        button: MouseButton::Left,
        modifiers: Modifiers::NONE,
    }
}

fn down_mod(x: i32, y: i32, modifiers: Modifiers) -> Event {
    Event::MouseDown {
        x,
        y,
        button: MouseButton::Left,
        modifiers,
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

fn row_y(index: i32) -> i32 {
    ROW.to_px(96).value() * index + ROW.to_px(96).value() / 2
}

/// Whether every text op is inside the innermost clip open when it was drawn.
fn every_text_is_clipped(ops: &[DrawOp]) -> bool {
    let mut clips: Vec<Rect> = Vec::new();
    for op in ops {
        match op {
            DrawOp::Clip(rect) => clips.push(*rect),
            DrawOp::Unclip => {
                clips.pop();
            }
            DrawOp::Text(rect, ..) => {
                let Some(clip) = clips.last() else {
                    return false;
                };
                if rect.left < clip.left
                    || rect.right > clip.right
                    || rect.top < clip.top
                    || rect.bottom > clip.bottom
                {
                    return false;
                }
            }
            _ => {}
        }
    }
    true
}

fn texts(backend: &HeadlessBackend, id: WidgetId) -> Vec<String> {
    backend
        .ops(id)
        .into_iter()
        .filter_map(|op| match op {
            DrawOp::Text(_, text, _) => Some(text),
            _ => None,
        })
        .collect()
}

fn text_rect(ops: &[DrawOp], needle: &str) -> Option<Rect> {
    ops.iter().find_map(|op| match op {
        DrawOp::Text(rect, text, _) if text == needle => Some(*rect),
        _ => None,
    })
}
