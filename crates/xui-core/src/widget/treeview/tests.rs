#![forbid(unsafe_code)]

//! Unit tests for the flat and model-backed [`TreeView`].

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use super::flatten::{
    checkbox_rect, chevron_hit, guide_continues, guide_x, is_visible, label_x, level_x,
    slot_to_index,
};
use super::*;
use crate::app::{App, Core, Runtime};
use crate::backend::headless::HeadlessBackend;
use crate::backend::{Backend, Event, PlatformSpec, WidgetId};
use crate::geometry::Point;
use crate::message::{Key, Modifiers, MouseButton};

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
            None => vec![TreeNode::branch(1, "Inbox"), TreeNode::leaf(2, "Sent")],
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

#[test]
fn clicking_a_row_selects_and_raises_a_message() {
    let (runtime, tree) = harness_flat();
    send(&runtime, &tree, &down(80, 5));
    assert_eq!(tree.selected(), Some(0));
    assert_eq!(log(), vec![0]);
}

#[test]
fn a_right_click_reports_the_row_and_pointer_position() {
    LOG.with(|log| log.borrow_mut().clear());
    let backend = Rc::new(HeadlessBackend::new());
    let window = backend.open_window(&PlatformSpec::new("t")).unwrap();
    let core = Core::new(backend.clone(), window);
    let ui = Ui::new(Rc::clone(&core));
    let at = Rc::new(Cell::new(None));
    let seen = Rc::clone(&at);
    let tree = TreeView::new(&ui, Rect::new(0, 0, 120, 88), &flat())
        .unwrap()
        .on_context(move |id, point| {
            seen.set(Some((id, point)));
            Some(7)
        });
    let runtime = Runtime::primary(core, TestApp);

    send(&runtime, &tree, &right(80, 5));

    assert_eq!(at.get(), Some((0, Point::new(80, 5))));
    assert_eq!(tree.selected(), Some(0));
}

#[test]
fn clicking_a_chevron_toggles_and_raises_a_message() {
    let (runtime, tree) = harness_flat();
    send(&runtime, &tree, &down(10, 11));
    send(&runtime, &tree, &down(10, 11));
    assert_eq!(tree.selected(), None, "a chevron click does not select");
    assert_eq!(log(), vec![101, 100]);
}

#[test]
fn the_arrow_keys_expand_then_collapse_a_selected_row() {
    let (runtime, tree) = harness_flat();
    tree.select(Some(0));
    send(&runtime, &tree, &key(Key::RIGHT));
    send(&runtime, &tree, &key(Key::LEFT));
    assert_eq!(log(), vec![101, 100]);
}

#[test]
fn a_model_loads_children_only_on_expand() {
    let (runtime, tree, loads) = harness_model(false);
    assert_eq!(loads.get(), 1, "only the roots load when the tree is built");
    assert_eq!(tree.len(), 2);

    send(&runtime, &tree, &down(10, 11));
    assert_eq!(loads.get(), 2, "expanding reads the branch once");
    assert_eq!(tree.len(), 4, "the two children are now materialized");
    assert_eq!(tree.selected(), None, "a chevron click does not select");

    send(&runtime, &tree, &down(10, 11));
    assert!(!is_visible(&tree.state.borrow().rows, 1));
    send(&runtime, &tree, &down(10, 11));
    assert_eq!(loads.get(), 2, "re-expanding reuses the loaded children");
    assert!(is_visible(&tree.state.borrow().rows, 1));
}

#[test]
fn a_collapsed_branch_hides_its_descendants() {
    let (runtime, tree, _) = harness_model(false);
    send(&runtime, &tree, &down(10, 11));
    {
        let state = tree.state.borrow();
        assert!(is_visible(&state.rows, 2), "an expanded chain shows a leaf");
        assert_eq!(slot_to_index(&state.rows, 2), Some(2));
    }

    send(&runtime, &tree, &down(10, 11));
    let state = tree.state.borrow();
    assert!(!is_visible(&state.rows, 1), "collapsing hides the children");
    assert!(!is_visible(&state.rows, 2));
    assert_eq!(
        slot_to_index(&state.rows, 1),
        Some(3),
        "Sent moves up a slot"
    );
    assert_eq!(slot_to_index(&state.rows, 2), None, "no third visible row");
}

#[test]
fn checkboxes_toggle_independently() {
    let (runtime, tree, _) = harness_model(false);
    send(&runtime, &tree, &down(10, 11));
    send(&runtime, &tree, &down(44, 33));
    assert_eq!(tree.checked(11), Some(CheckState::Checked));
    assert_eq!(tree.checked(12), Some(CheckState::Unchecked));
    assert_eq!(tree.selected(), None, "a checkbox click does not select");

    send(&runtime, &tree, &down(44, 55));
    assert_eq!(tree.checked(12), Some(CheckState::Checked));
    assert_eq!(
        tree.checked(11),
        Some(CheckState::Checked),
        "rows are independent"
    );
    assert_eq!(
        log(),
        vec![203, 300 + 11 * 2 + 1, 300 + 12 * 2 + 1],
        "the expand and both check messages"
    );
}

#[test]
fn a_tri_state_checkbox_visits_the_mixed_state() {
    let (runtime, tree, _) = harness_model(true);
    send(&runtime, &tree, &down(10, 11));
    send(&runtime, &tree, &down(44, 33));
    assert_eq!(tree.checked(11), Some(CheckState::Checked));
    send(&runtime, &tree, &down(44, 33));
    assert_eq!(tree.checked(11), Some(CheckState::Indeterminate));
    send(&runtime, &tree, &down(44, 33));
    assert_eq!(tree.checked(11), Some(CheckState::Unchecked));
}

#[test]
fn keyboard_navigation_skips_hidden_rows() {
    let (runtime, tree, _) = harness_model(false);
    tree.select(Some(2));
    send(&runtime, &tree, &key(Key::UP));
    assert_eq!(tree.selected(), Some(1), "the hidden children are skipped");
    send(&runtime, &tree, &key(Key::DOWN));
    assert_eq!(tree.selected(), Some(2));

    tree.select(Some(1));
    send(&runtime, &tree, &key(Key::RIGHT));
    send(&runtime, &tree, &key(Key::DOWN));
    assert_eq!(tree.selected(), Some(11));
    send(&runtime, &tree, &key(Key::DOWN));
    assert_eq!(tree.selected(), Some(12));
    send(&runtime, &tree, &key(Key::DOWN));
    assert_eq!(tree.selected(), Some(2));
    send(&runtime, &tree, &key(Key::UP));
    assert_eq!(tree.selected(), Some(12));
}

#[test]
fn home_and_end_choose_visible_rows() {
    let (runtime, tree, _) = harness_model(false);
    tree.select(Some(2));
    send(&runtime, &tree, &key(Key::HOME));
    assert_eq!(tree.selected(), Some(1));
    send(&runtime, &tree, &key(Key::END));
    assert_eq!(tree.selected(), Some(2));
}

#[test]
fn space_toggles_the_selected_rows_checkbox() {
    let (runtime, tree, _) = harness_model(false);
    tree.select(Some(2));
    send(&runtime, &tree, &key(Key::SPACE));
    assert_eq!(tree.checked(2), Some(CheckState::Checked));
    assert_eq!(
        tree.selected(),
        Some(2),
        "Space does not move the selection"
    );
}

#[test]
fn indent_and_chevron_geometry_is_consistent() {
    assert_eq!(level_x(96, 0, 0), 4, "a root starts after the padding");
    assert_eq!(level_x(96, 0, 1), 20, "a level adds one indent step");
    assert_eq!(level_x(96, 0, 2), 36);

    assert!(chevron_hit(96, 0, 0, true, 10), "inside the chevron slot");
    assert!(!chevron_hit(96, 0, 0, true, 2), "left of the slot");
    assert!(!chevron_hit(96, 0, 0, true, 20), "right of the slot");
    assert!(
        !chevron_hit(96, 0, 0, false, 10),
        "a leaf has no chevron to hit"
    );

    assert_eq!(label_x(96, 0, 1, false), 36, "the label clears the chevron");
    assert_eq!(
        label_x(96, 0, 1, true),
        36 + 16 + 6,
        "a checkbox and its gap push the label right"
    );

    let square = checkbox_rect(96, 0, 22, 1);
    assert_eq!((square.left, square.top), (36, 25));
    assert!(square.contains(Point::new(44, 33)), "inside the checkbox");

    assert_eq!(guide_x(96, 0, 1), 20 + 8, "a guide is under its chevron");
}

#[test]
fn an_indent_guide_stops_after_the_last_child() {
    let rows = [
        TreeRow::new("Inbox", 0).expandable(true).expanded(true),
        TreeRow::new("Work", 1),
        TreeRow::new("Home", 1),
        TreeRow::new("Archive", 0).expandable(true).expanded(true),
        TreeRow::new("Old", 1),
    ];
    let state = super::flatten::State::flat(&rows);
    assert!(
        guide_continues(&state.rows, 1, 0),
        "Inbox has a sibling after"
    );
    assert!(guide_continues(&state.rows, 2, 0), "so does Archive");
    assert!(
        !guide_continues(&state.rows, 4, 0),
        "Archive is the last root, so Old ends the guide"
    );
    assert!(
        !guide_continues(&state.rows, 2, 1),
        "Home is the last child at its level"
    );
}
