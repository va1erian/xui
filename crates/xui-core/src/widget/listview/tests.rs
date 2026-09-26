#![forbid(unsafe_code)]

//! Unit tests for [`ListView`](super::ListView): virtualization, columns,
//! selection modes, the sort and context hooks and keyboard navigation.

use std::cell::RefCell;
use std::rc::Rc;

use super::state::ROW;
use super::{CellData, ListModel, ListView, SelectionMode};
use crate::app::{App, Core, Runtime, Ui};
use crate::backend::headless::{DrawOp, HeadlessBackend};
use crate::backend::{Backend, Event, PlatformSpec, WidgetId};
use crate::geometry::Rect;
use crate::message::{Key, Modifiers, MouseButton};
use crate::units::Dip;

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

#[test]
fn clicking_a_row_selects_and_raises_a_message() {
    let (_backend, core, ui) = setup();
    let list = ListView::new(&ui, Rect::new(0, 0, 120, 88), &["one", "two", "three"])
        .unwrap()
        .on_select(|index| Some(index as u32));
    let runtime = Runtime::primary(core, TestApp(log()));

    runtime.deliver(list.id(), &down(5, row_y(1)));
    runtime.deliver(WidgetId::NONE, &Event::Wake);

    assert_eq!(list.selected(), Some(1));
}

#[test]
fn a_hit_test_uses_node_local_coordinates() {
    let (_backend, core, ui) = setup();
    let list = ListView::new(&ui, Rect::new(40, 200, 160, 288), &["one", "two", "three"])
        .unwrap()
        .on_select(|index| Some(index as u32));
    let runtime = Runtime::primary(core, TestApp(log()));

    runtime.deliver(list.id(), &down(5, row_y(1)));
    runtime.deliver(WidgetId::NONE, &Event::Wake);

    assert_eq!(list.selected(), Some(1));
}

#[test]
fn down_then_return_activates_a_row() {
    let (_backend, core, ui) = setup();
    let log = log();
    let list = ListView::new(&ui, Rect::new(0, 0, 120, 88), &["one", "two", "three"])
        .unwrap()
        .on_activate(|index| Some(index as u32));
    let runtime = Runtime::primary(core, TestApp(Rc::clone(&log)));

    runtime.deliver(list.id(), &key(Key::DOWN));
    runtime.deliver(list.id(), &key(Key::RETURN));
    runtime.deliver(WidgetId::NONE, &Event::Wake);

    assert_eq!(list.selected(), Some(1));
    assert_eq!(*log.borrow(), vec![1]);
}

#[test]
fn select_is_programmatic_and_raises_nothing() {
    let (_backend, core, ui) = setup();
    let log = log();
    let list = ListView::new(&ui, Rect::new(0, 0, 120, 88), &["one", "two", "three"])
        .unwrap()
        .on_select(|index| Some(index as u32));
    let runtime = Runtime::primary(core, TestApp(Rc::clone(&log)));

    list.select(Some(2));
    runtime.deliver(WidgetId::NONE, &Event::Wake);

    assert_eq!(list.selected(), Some(2));
    assert!(log.borrow().is_empty());
}

#[test]
fn a_large_model_renders_only_visible_rows() {
    let (backend, _core, ui) = setup();
    let model: Vec<String> = (0..1000).map(|index| format!("row {index}")).collect();
    let list = ListView::with_model(&ui, Rect::new(0, 0, 200, 88), model).unwrap();

    backend.render(list.id());
    assert_eq!(
        texts(&backend, list.id()),
        vec!["row 0", "row 1", "row 2", "row 3"]
    );
}

#[test]
fn the_wheel_scrolls_the_virtual_window() {
    let (backend, core, ui) = setup();
    let model: Vec<String> = (0..1000).map(|index| format!("row {index}")).collect();
    let list = ListView::with_model(&ui, Rect::new(0, 0, 200, 88), model).unwrap();
    let runtime = Runtime::primary(core, TestApp(log()));

    runtime.deliver(
        list.id(),
        &Event::MouseWheel {
            delta: -1,
            horizontal: false,
            x: 5,
            y: 5,
            modifiers: Modifiers::NONE,
        },
    );
    runtime.deliver(WidgetId::NONE, &Event::Wake);
    backend.render(list.id());

    assert_eq!(
        texts(&backend, list.id()),
        vec!["row 3", "row 4", "row 5", "row 6"]
    );
}

#[test]
fn columns_with_a_fixed_and_a_fill_width_align() {
    let (backend, _core, ui) = setup();
    let model: Vec<Vec<String>> = vec![
        vec!["a0".into(), "b0".into()],
        vec!["a1".into(), "b1".into()],
    ];
    let list = ListView::with_model(&ui, Rect::new(0, 0, 300, 120), model)
        .unwrap()
        .column("A", Dip::new(100.0))
        .column("B", super::Fill);

    backend.render(list.id());
    let ops = backend.ops(list.id());
    // The fill column begins right after the 100px fixed column (plus inset).
    let b0 = text_rect(&ops, "b0").expect("second column cell");
    let a0 = text_rect(&ops, "a0").expect("first column cell");
    assert_eq!(a0.left, 6);
    assert_eq!(b0.left, 106);
    assert!(text_rect(&ops, "B").is_some(), "the header is drawn");
}

#[test]
fn cell_text_is_clipped_to_its_column() {
    let (backend, _core, ui) = setup();
    let model: Vec<Vec<String>> = vec![
        vec!["a title far too long for its column".into(), "b".into()],
        vec!["short".into(), "c".into()],
    ];
    let list = ListView::with_model(&ui, Rect::new(0, 0, 200, 120), model)
        .unwrap()
        .column("A", Dip::new(60.0))
        .column("B", super::Fill);

    backend.render(list.id());
    assert!(
        every_text_is_clipped(&backend.ops(list.id())),
        "a cell's text must sit inside the clip pushed for its column"
    );
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

#[test]
fn multi_select_toggles_and_shift_extends_from_the_anchor() {
    let (_backend, core, ui) = setup();
    let items = ["a", "b", "c", "d", "e"];
    let list = ListView::new(&ui, Rect::new(0, 0, 200, 200), &items)
        .unwrap()
        .selection_mode(SelectionMode::Multi)
        .on_selection(|rows| Some(rows.len() as u32));
    let runtime = Runtime::primary(core, TestApp(log()));

    runtime.deliver(list.id(), &down(5, row_y(1)));
    assert_eq!(list.selection(), vec![1]);
    runtime.deliver(list.id(), &down_mod(5, row_y(3), ctrl()));
    assert_eq!(list.selection(), vec![1, 3]);
    runtime.deliver(list.id(), &down_mod(5, row_y(1), ctrl()));
    assert_eq!(list.selection(), vec![3]);
    runtime.deliver(list.id(), &down_mod(5, row_y(0), shift()));
    assert_eq!(list.selection(), vec![0, 1]);
    runtime.deliver(WidgetId::NONE, &Event::Wake);
}

#[test]
fn the_range_mode_extends_from_the_anchor() {
    let (_backend, core, ui) = setup();
    let items = ["a", "b", "c", "d", "e"];
    let list = ListView::new(&ui, Rect::new(0, 0, 200, 200), &items)
        .unwrap()
        .selection_mode(SelectionMode::Range)
        .on_selection(|rows| Some(rows.len() as u32));
    let runtime = Runtime::primary(core, TestApp(log()));

    runtime.deliver(list.id(), &down(5, row_y(1)));
    runtime.deliver(list.id(), &down_mod(5, row_y(3), shift()));
    assert_eq!(list.selection(), vec![1, 2, 3]);
    runtime.deliver(WidgetId::NONE, &Event::Wake);
}

#[test]
fn a_header_click_toggles_the_sort_and_calls_the_hook() {
    let (_backend, core, ui) = setup();
    let log = log();
    let model: Vec<Vec<String>> = vec![vec!["a".into(), "b".into()]];
    let list = ListView::with_model(&ui, Rect::new(0, 0, 300, 120), model)
        .unwrap()
        .column("A", super::Fill)
        .column("B", Dip::new(50.0))
        .on_sort(|column| Some(column as u32));
    let runtime = Runtime::primary(core, TestApp(Rc::clone(&log)));

    runtime.deliver(list.id(), &down(275, 12));
    assert_eq!(
        list.sort_indicator(),
        Some((1, super::SortDirection::Ascending))
    );
    runtime.deliver(list.id(), &down(275, 12));
    assert_eq!(
        list.sort_indicator(),
        Some((1, super::SortDirection::Descending))
    );
    runtime.deliver(WidgetId::NONE, &Event::Wake);

    assert_eq!(*log.borrow(), vec![1, 1]);
}

#[test]
fn a_right_click_calls_the_context_hook() {
    let (_backend, core, ui) = setup();
    let log = log();
    let list = ListView::new(&ui, Rect::new(0, 0, 200, 200), &["a", "b", "c"])
        .unwrap()
        .on_context(|row| Some(100 + row as u32));
    let runtime = Runtime::primary(core, TestApp(Rc::clone(&log)));

    runtime.deliver(list.id(), &right(5, row_y(1)));
    runtime.deliver(WidgetId::NONE, &Event::Wake);

    assert_eq!(*log.borrow(), vec![101]);
}

#[test]
fn the_keyboard_navigates_and_space_toggles_in_multi() {
    let (_backend, core, ui) = setup();
    let log = log();
    let list = ListView::new(&ui, Rect::new(0, 0, 200, 200), &["a", "b", "c", "d", "e"])
        .unwrap()
        .selection_mode(SelectionMode::Multi)
        .on_selection(|rows| Some(rows.len() as u32));
    let runtime = Runtime::primary(core, TestApp(Rc::clone(&log)));

    runtime.deliver(list.id(), &key(Key::DOWN));
    assert_eq!(list.focused(), Some(1));
    assert_eq!(list.selection(), vec![1]);
    runtime.deliver(list.id(), &key(Key::SPACE));
    assert_eq!(list.selection(), vec![]);
    runtime.deliver(list.id(), &key(Key::SPACE));
    assert_eq!(list.selection(), vec![1]);
    runtime.deliver(list.id(), &key(Key::END));
    assert_eq!(list.selection(), vec![4]);
    runtime.deliver(list.id(), &key(Key::HOME));
    assert_eq!(list.selection(), vec![0]);
    runtime.deliver(list.id(), &key(Key::PAGE_DOWN));
    assert_eq!(list.selection(), vec![4]);
    runtime.deliver(list.id(), &key(Key::UP));
    assert_eq!(list.selection(), vec![3]);
    runtime.deliver(WidgetId::NONE, &Event::Wake);

    assert!(!log.borrow().is_empty());
}

#[test]
fn a_model_can_attach_cell_data() {
    struct Row {
        text: &'static str,
        data: CellData,
    }
    struct Model(Vec<Row>);

    impl ListModel for Model {
        fn rows(&self) -> usize {
            self.0.len()
        }

        fn cell(&self, row: usize, _column: usize) -> Option<&str> {
            self.0.get(row).map(|row| row.text)
        }

        fn data(&self, row: usize, _column: usize) -> Option<CellData> {
            self.0.get(row).map(|row| Rc::clone(&row.data))
        }
    }

    let (_backend, _core, ui) = setup();
    let model = Model(vec![
        Row {
            text: "one",
            data: Rc::new(41u32),
        },
        Row {
            text: "two",
            data: Rc::new(42u32),
        },
    ]);
    let list = ListView::with_model(&ui, Rect::new(0, 0, 200, 120), model)
        .unwrap()
        .column("A", super::Fill);

    assert_eq!(list.cell_text(1, 0).as_deref(), Some("two"));
    let data = list.cell_data(1, 0).expect("data");
    assert_eq!(data.downcast_ref::<u32>(), Some(&42));
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
