//! Selecting tiles: single, multi and range modes, plus clearing and model
//! replacement.

use super::*;
use crate::widget::SelectionMode;

#[test]
fn clicking_a_tile_selects_it_and_raises_a_message() {
    let (_backend, core, ui) = setup();
    let items = view(&ui, 6).on_select(|index| Some(index as u32));
    let runtime = Runtime::primary(core, TestApp(log()));
    let point = center(&items, 2);

    runtime.deliver(items.id(), &left(point.x, point.y));
    runtime.deliver(WidgetId::NONE, &Event::Wake);

    assert_eq!(items.selected(), Some(2));
}

#[test]
fn clicking_empty_space_clears_the_selection() {
    let (_backend, core, ui) = setup();
    let items = view(&ui, 3).on_selection(|rows| Some(rows.len() as u32));
    let runtime = Runtime::primary(core, TestApp(log()));

    let first = center(&items, 0);
    runtime.deliver(items.id(), &left(first.x, first.y));
    assert_eq!(items.selected(), Some(0));
    let empty = empty_point();
    runtime.deliver(items.id(), &left(empty.x, empty.y));
    assert_eq!(items.selection(), Vec::<usize>::new());
    assert_eq!(items.focused(), None);
}

#[test]
fn multi_select_toggles_and_shift_extends_across_rows() {
    let (_backend, core, ui) = setup();
    let items = view(&ui, 8)
        .multi_select(true)
        .on_selection(|rows| Some(rows.len() as u32));
    let runtime = Runtime::primary(core, TestApp(log()));

    let p1 = center(&items, 1);
    let p2 = center(&items, 2);
    let p4 = center(&items, 4);
    runtime.deliver(items.id(), &left(p1.x, p1.y));
    assert_eq!(items.selection(), vec![1]);
    runtime.deliver(items.id(), &left_mod(p4.x, p4.y, shift()));
    assert_eq!(items.selection(), vec![1, 2, 3, 4], "shift spans rows");
    runtime.deliver(items.id(), &left_mod(p2.x, p2.y, ctrl()));
    assert_eq!(items.selection(), vec![1, 3, 4], "ctrl toggles one out");
}

#[test]
fn the_range_mode_extends_from_the_anchor() {
    let (_backend, core, ui) = setup();
    let items = view(&ui, 8)
        .selection_mode(SelectionMode::Range)
        .on_selection(|rows| Some(rows.len() as u32));
    let runtime = Runtime::primary(core, TestApp(log()));

    let p1 = center(&items, 1);
    let p3 = center(&items, 3);
    runtime.deliver(items.id(), &left(p1.x, p1.y));
    runtime.deliver(items.id(), &left_mod(p3.x, p3.y, shift()));
    assert_eq!(items.selection(), vec![1, 2, 3]);
}

#[test]
fn select_and_set_selection_raise_no_message() {
    let (_backend, core, ui) = setup();
    let log = log();
    let items = view(&ui, 6).on_selection(|rows| Some(rows.len() as u32));
    let runtime = Runtime::primary(core, TestApp(Rc::clone(&log)));

    items.select(Some(4));
    assert_eq!(items.selected(), Some(4));
    items.set_selection(&[1, 3]);
    assert_eq!(items.selection(), vec![1, 3]);
    items.set_selection(&[9, 9, 2]);
    assert_eq!(items.selection(), vec![2]);
    runtime.deliver(WidgetId::NONE, &Event::Wake);
    assert!(log.borrow().is_empty());
}

#[test]
fn replacing_the_model_drops_out_of_range_state() {
    let (_backend, _core, ui) = setup();
    let items = view(&ui, 10);
    items.select(Some(8));
    items.state.borrow_mut().offset = 500;
    items.set_items(&["a", "b", "c"]);

    assert_eq!(items.len(), 3);
    assert!(items.selection().iter().all(|item| *item < 3));
    assert!(items.focused().is_some_and(|item| item < 3));
    assert_eq!(items.state.borrow().offset, 0);
}

#[test]
fn changing_the_icon_size_keeps_the_focus_and_clamps_the_scroll() {
    let (_backend, _core, ui) = setup();
    let items = view(&ui, 60);
    items.select(Some(59));
    items.ensure_visible(59);
    let before = items.focused();
    assert!(items.state.borrow().offset > 0, "scrolled to the last item");

    items.set_icon_size(IconSize::Small);
    assert_eq!(items.icon_size(), IconSize::Small);
    assert_eq!(items.focused(), before);
    let state = items.state.borrow();
    let max = state.max_offset(Rect::new(0, 0, WIDTH, HEIGHT), 96);
    assert!(state.offset <= max && state.offset >= 0);
}

#[test]
fn the_selection_mode_is_configurable() {
    let (_backend, _core, ui) = setup();
    let items = view(&ui, 3).selection_mode(SelectionMode::Range);
    assert_eq!(items.state.borrow().mode, SelectionMode::Range);
}
