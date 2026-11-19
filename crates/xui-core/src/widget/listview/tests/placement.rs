//! Placement: a layout moves the list's scrollbar with it, and a live refresh
//! keeps the scroll position.

use super::*;
use crate::arrange::{Handle, LayoutExt as _, column, list, tabs};
use crate::widget::Tabs;

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

/// A list on the second page of a tab control, with 100 rows, and the first
/// page selected so the list has no size yet.
type HiddenList = (
    Rc<Core<u32>>,
    Ui<u32>,
    Handle<ListView<u32>>,
    Handle<Tabs<u32>>,
);

fn hidden_list() -> HiddenList {
    let (_backend, core, ui) = setup();
    let table = Handle::<ListView<u32>>::new();
    let pages = Handle::<Tabs<u32>>::new();
    let mounted = ui
        .mount(
            column().child(
                tabs()
                    .page("One", column())
                    .page(
                        "Two",
                        column().child(list().column("Row", super::Fill).bind(&table).fill(1)),
                    )
                    .bind(&pages)
                    .fill(1),
            ),
        )
        .unwrap();
    std::mem::forget(mounted);
    table.get().set_model(rows(100));
    assert_eq!(
        ui.bounds(table.get().id()).height(),
        0,
        "a hidden page has no size"
    );
    (core, ui, table, pages)
}

/// How many whole rows the shown list has room for.
fn visible_rows_of(table: &ListView<u32>, ui: &Ui<u32>) -> usize {
    let state = table.state.borrow();
    let body = ui.bounds(table.id()).height()
        - crate::widget::listview::state::header_px(state.has_header(), ui.dpi());
    assert!(body > 0, "the page gave the list its size");
    (body / ROW.to_px(ui.dpi()).value()) as usize
}

#[test]
fn ensure_visible_of_the_last_row_in_a_hidden_tab_page_ends_the_view() {
    let (core, ui, table, pages) = hidden_list();
    let table = table.get();
    table.ensure_visible(99);

    pages.get().select(1);
    core.flush_layout();

    let visible = visible_rows_of(&table, &ui);
    assert!(visible > 0 && visible < 100, "{visible} rows fit");
    assert_eq!(
        table.state.borrow().offset,
        100 - visible,
        "row 99 is the last visible row, not scrolled past the end"
    );
}

#[test]
fn ensure_visible_of_a_row_that_fits_in_a_hidden_tab_page_does_not_scroll() {
    let (core, ui, table, pages) = hidden_list();
    let table = table.get();
    table.ensure_visible(5);

    pages.get().select(1);
    core.flush_layout();

    assert!(
        visible_rows_of(&table, &ui) > 5,
        "row 5 fits without scrolling"
    );
    assert_eq!(
        table.state.borrow().offset,
        0,
        "scroll just enough: not at all"
    );
}

#[test]
fn a_middle_row_asked_for_in_a_hidden_tab_page_is_revealed_when_it_is_shown() {
    let (core, ui, table, pages) = hidden_list();
    let table = table.get();
    table.ensure_visible(60);

    pages.get().select(1);
    core.flush_layout();

    let visible = visible_rows_of(&table, &ui);
    let offset = table.state.borrow().offset;
    assert!(
        (offset..offset + visible).contains(&60),
        "row 60 is shown: offset {offset}, {visible} rows visible"
    );
}
