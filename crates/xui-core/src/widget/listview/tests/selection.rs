//! Selecting rows: single, multi and range modes.

use super::*;

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
fn double_clicking_a_row_activates_it_and_empty_space_does_not() {
    let (_backend, core, ui) = setup();
    let log = log();
    let list = ListView::new(&ui, Rect::new(0, 0, 120, 200), &["one", "two"])
        .unwrap()
        .on_activate(|index| Some(index as u32 + 10));
    let runtime = Runtime::primary(core, TestApp(Rc::clone(&log)));
    let double = |y| Event::MouseDoubleClick {
        x: 5,
        y,
        button: MouseButton::Left,
        modifiers: Modifiers::NONE,
    };

    runtime.deliver(list.id(), &double(row_y(1)));
    runtime.deliver(list.id(), &double(row_y(5)));
    runtime.deliver(WidgetId::NONE, &Event::Wake);

    assert_eq!(*log.borrow(), vec![11]);
}
