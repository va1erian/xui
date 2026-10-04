//! Placement: a layout moves the list's scrollbar with it, and a live refresh
//! keeps the scroll position.

use super::*;
use crate::arrange::{Handle, LayoutExt as _, column, list};

fn rows(count: usize) -> Vec<Vec<String>> {
    (0..count)
        .map(|index| vec![format!("row {index}")])
        .collect()
}

#[test]
fn a_layout_placement_moves_the_scrollbar_with_the_list() {
    let (_backend, _core, ui) = setup();
    let table = Handle::<ListView<u32>>::new();
    let _mounted = ui
        .mount(column().child(list().column("Row", super::Fill).bind(&table).fill(1)))
        .unwrap();
    let table = table.get();
    table.set_model(rows(100));

    let list_bounds = ui.bounds(table.id());
    let bar = ui.bounds(table.bar.id());
    assert_eq!(bar.right, list_bounds.right, "hugs the trailing edge");
    assert_eq!(bar.bottom, list_bounds.bottom);
    assert!(bar.width() > 0, "the rows overflow, so the bar is shown");
}

#[test]
fn a_refresh_keeps_the_scroll_position_and_set_model_resets_it() {
    let (_backend, _core, ui) = setup();
    let table = Handle::<ListView<u32>>::new();
    let _mounted = ui
        .mount(column().child(list().column("Row", super::Fill).bind(&table).fill(1)))
        .unwrap();
    let table = table.get();
    table.set_model(rows(100));
    table.state.borrow_mut().offset = 40;

    table.refresh_model(rows(100));
    assert_eq!(table.state.borrow().offset, 40);
    table.refresh_model(rows(10));
    assert_eq!(table.state.borrow().offset, 9, "clamped to the new rows");
    table.set_model(rows(100));
    assert_eq!(table.state.borrow().offset, 0);
}
