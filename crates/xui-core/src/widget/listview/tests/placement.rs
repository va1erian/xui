//! Placement: a layout moves the list's scrollbar with it.

use super::*;

#[test]
fn a_layout_placement_moves_the_scrollbar_with_the_list() {
    use crate::arrange::{LayoutExt as _, column};

    let (_backend, _core, ui) = setup();
    let rows: Vec<String> = (0..100).map(|index| format!("row {index}")).collect();
    let list = Rc::new(
        ListView::auto(&ui, rows)
            .unwrap()
            .column("Row", super::Fill),
    );
    let _mounted = ui.mount(column().child(list.fill(1))).unwrap();

    let list_bounds = ui.bounds(list.id());
    let bar = ui.bounds(list.bar.id());
    assert_eq!(bar.right, list_bounds.right, "hugs the trailing edge");
    assert_eq!(bar.bottom, list_bounds.bottom);
    assert!(bar.width() > 0, "the rows overflow, so the bar is shown");
}
