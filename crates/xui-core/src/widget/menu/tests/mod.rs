//! Unit tests for [`Menu`](super::Menu).

use std::cell::RefCell;
use std::rc::Rc;

use super::{Menu, MenuId, layout};
use crate::app::{App, Core, Runtime, Ui};
use crate::backend::headless::{DrawOp, HeadlessBackend};
use crate::backend::{Backend, Event, PlatformSpec, WidgetId};
use crate::geometry::Rect;
use crate::message::{Key, Modifiers, MouseButton};

mod items;
mod keyboard;
mod opening;
mod painting;
mod surfaces;

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

fn down(x: i32, y: i32) -> Event {
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

fn visible(backend: &HeadlessBackend, id: WidgetId) -> bool {
    backend.node(id).expect("node").3
}

/// The row geometry a popup lays out its entries with.
fn geometry(dpi: u32) -> (i32, i32, i32) {
    (
        layout::ROW.to_px(dpi).value(),
        layout::SEPARATOR_ROW.to_px(dpi).value(),
        layout::PAD.to_px(dpi).value(),
    )
}

fn sample_bar(ui: &Ui<u32>) -> Menu<u32> {
    Menu::bar(ui, Rect::new(0, 0, 260, 28))
        .unwrap()
        .on_select(|id| Some(id.0 as u32))
        .build(|m| {
            m.submenu(MenuId::new(1), "&File", |f| {
                f.item(MenuId::new(2), "&New");
                f.item(MenuId::new(3), "&Open");
            });
            m.submenu(MenuId::new(4), "&Edit", |e| {
                e.item(MenuId::new(5), "Cu&t");
            });
        })
}
