#![forbid(unsafe_code)]

//! Unit tests for the flat and model-backed [`TreeView`].

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use super::flatten::{
    checkbox_rect, chevron_hit, guide_continues, guide_x, icon_rect, icon_x, is_visible, label_x,
    level_x, slot_to_index,
};
use super::paint::guide_color;
use super::*;
use crate::Image;
use crate::app::{App, Core, Runtime};
use crate::backend::headless::{DrawOp, HeadlessBackend};
use crate::backend::{Backend, Event, PlatformSpec, WidgetId};
use crate::geometry::{Point, Rect};
use crate::message::{Key, Modifiers, MouseButton};
use crate::theme::Theme;
use crate::widget::Glyph;

mod checkboxes;
mod geometry;
mod model;
mod navigation;
mod paint;
mod selection;

thread_local! {
    static LOG: RefCell<Vec<u32>> = const { RefCell::new(Vec::new()) };
}

struct TestApp;

impl App for TestApp {
    type Msg = u32;
    fn update(&mut self, msg: u32, _ui: &mut Ui<u32>) {
        LOG.with(|log| log.borrow_mut().push(msg));
    }
}

/// A model whose `loads` count proves children are only read on expansion.
struct Folders {
    loads: Rc<Cell<usize>>,
}

impl TreeModel for Folders {
    fn children(&self, parent: Option<NodeId>) -> Vec<TreeNode> {
        self.loads.set(self.loads.get() + 1);
        match parent {
            None => vec![
                TreeNode::branch(1, "Inbox").icon(Glyph::Folder),
                TreeNode::leaf(2, "Sent"),
            ],
            Some(1) => vec![TreeNode::leaf(11, "Work"), TreeNode::leaf(12, "Home")],
            _ => Vec::new(),
        }
    }
}

fn flat() -> [TreeRow; 2] {
    [
        TreeRow::new("root", 0).expandable(true),
        TreeRow::new("leaf", 1),
    ]
}

fn harness_flat() -> (Rc<Runtime<TestApp>>, TreeView<u32>) {
    LOG.with(|log| log.borrow_mut().clear());
    let backend = Rc::new(HeadlessBackend::new());
    let window = backend.open_window(&PlatformSpec::new("t")).unwrap();
    let core = Core::new(backend.clone(), window);
    let ui = Ui::new(Rc::clone(&core));
    let tree = TreeView::new(&ui, Rect::new(0, 0, 120, 88), &flat())
        .unwrap()
        .on_select(|id| Some(id as u32))
        .on_toggle(|id, e| Some(100 + id as u32 * 2 + e as u32));
    (Runtime::primary(core, TestApp), tree)
}

fn harness_model(tri_state: bool) -> (Rc<Runtime<TestApp>>, TreeView<u32>, Rc<Cell<usize>>) {
    LOG.with(|log| log.borrow_mut().clear());
    let backend = Rc::new(HeadlessBackend::new());
    let window = backend.open_window(&PlatformSpec::new("t")).unwrap();
    let core = Core::new(backend.clone(), window);
    let ui = Ui::new(Rc::clone(&core));
    let loads = Rc::new(Cell::new(0));
    let tree = TreeView::with_model(
        &ui,
        Rect::new(0, 0, 160, 88),
        Folders {
            loads: Rc::clone(&loads),
        },
    )
    .unwrap()
    .checkboxes(true)
    .tri_state(tri_state)
    .on_select(|id| Some(id as u32))
    .on_toggle(|id, e| Some(200 + id as u32 * 2 + e as u32))
    .on_check(|id, state| Some(300 + id as u32 * 2 + code(state)));
    (Runtime::primary(core, TestApp), tree, loads)
}

fn code(state: CheckState) -> u32 {
    match state {
        CheckState::Unchecked => 0,
        CheckState::Checked => 1,
        CheckState::Indeterminate => 2,
    }
}

fn down(x: i32, y: i32) -> Event {
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

fn key(key: Key) -> Event {
    Event::KeyDown {
        key,
        modifiers: Modifiers::NONE,
        repeat: 1,
        system: false,
    }
}

fn send(runtime: &Rc<Runtime<TestApp>>, tree: &TreeView<u32>, event: &Event) {
    runtime.deliver(tree.id(), event);
    runtime.deliver(WidgetId::NONE, &Event::Wake);
}

fn log() -> Vec<u32> {
    LOG.with(|log| log.borrow().clone())
}
