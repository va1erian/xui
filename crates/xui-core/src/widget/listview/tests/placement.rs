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

    table.select(None);
    table.refresh_model(rows(100));
    assert_eq!(table.state.borrow().offset, 40);
    assert_eq!(table.selected(), None, "a refresh selects nothing");
    table.select(Some(5));
    table.refresh_model(rows(100));
    assert_eq!(table.selected(), Some(5), "and keeps the selection");
    table.refresh_model(rows(10));
    assert_eq!(table.state.borrow().offset, 9, "clamped to the new rows");
    table.set_model(rows(100));
    assert_eq!(table.state.borrow().offset, 0);
}

#[test]
fn reordering_carries_the_selection_with_its_items() {
    let (_backend, _core, ui) = setup();
    let table = Handle::<ListView<u32>>::new();
    let _mounted = ui
        .mount(column().child(list().column("Row", super::Fill).bind(&table).fill(1)))
        .unwrap();
    let table = table.get();
    table.set_model(rows(100));
    table.set_selection(&[2, 5]);

    // Reverse the rows: old row `i` becomes `99 - i`.
    let mut reversed = rows(100);
    reversed.reverse();
    table.reorder_model(reversed, |old| Some(99 - old));

    assert_eq!(table.selection(), vec![94, 97]);
    assert_eq!(table.cell_text(94, 0).as_deref(), Some("row 5"));
    assert_eq!(table.focused(), Some(97));
    assert!(
        table.state.borrow().offset > 0,
        "the focus is scrolled into view"
    );

    table.reorder_model(rows(3), |_| None);
    assert!(
        table.selection().is_empty(),
        "items that are gone are dropped"
    );
}
