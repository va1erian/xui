//! Radio groups, trees, colour choosers, panels and scroll views in layouts:
//! each measures from its content, so a widget squeezed once grows back.

use super::{bounds, setup};
use crate::arrange::{
    Handle, LayoutExt, build, button, color_panel, color_picker, column, panel, radio_group, row,
    tree_view,
};
use crate::geometry::Rect;
use crate::units::Dip;
use crate::widget::{
    BASIC_COLORS, Button, ColorPanel, ColorPicker, Panel, RadioGroup, ScrollView, TreeRow, TreeView,
};

#[test]
fn a_radio_group_stacks_its_options_and_grows_back_after_a_squeeze() {
    let (backend, window, ui, _runtime) = setup();
    let (group, after) = (
        Handle::<RadioGroup<u32>>::new(),
        Handle::<Button<u32>>::new(),
    );
    let _mounted = ui
        .mount(
            column().padding(4).children((
                radio_group(&["One", "Two", "Three"])
                    .selected(1)
                    .bind(&group),
                button("after").bind(&after),
            )),
        )
        .unwrap();
    let ids = group.get().ids();
    assert_eq!(ui.bounds(ids[0]), Rect::new(4, 4, 396, 32));
    assert_eq!(ui.bounds(ids[2]), Rect::new(4, 60, 396, 88));
    assert_eq!(bounds(&ui, &after).top, 88, "three 28-dip rows");
    assert_eq!(group.get().selected(), 1);

    backend.resize_window(window, 40, 20);
    backend.resize_window(window, 400, 300);
    assert_eq!(
        ui.bounds(ids[2]),
        Rect::new(4, 60, 396, 88),
        "not stuck small"
    );
}

#[test]
fn a_tree_view_and_a_colour_picker_measure_their_content() {
    let (_backend, _window, ui, _runtime) = setup();
    let (tree, picker) = (
        Handle::<TreeView<u32>>::new(),
        Handle::<ColorPicker<u32>>::new(),
    );
    let _mounted = ui
        .mount(
            row().children((
                tree_view()
                    .rows(vec![TreeRow::new("root", 0), TreeRow::new("leaf", 1)])
                    .bind(&tree),
                color_picker(&BASIC_COLORS[..10]).columns(5).bind(&picker),
            )),
        )
        .unwrap();
    assert_eq!(tree.get().len(), 2);
    assert_eq!(bounds(&ui, &tree).width(), 240);
    let swatches = bounds(&ui, &picker);
    assert_eq!((swatches.left, swatches.width()), (240, 5 * 32));
}

#[test]
fn a_panel_holds_its_layout_and_is_as_big_as_it() {
    let (backend, _window, ui, _runtime) = setup();
    let (card, inside) = (Handle::<Panel<u32>>::new(), Handle::<Button<u32>>::new());
    let _mounted = ui
        .mount(
            column().child(
                panel(column().padding(10).child(button("inside").bind(&inside))).bind(&card),
            ),
        )
        .unwrap();
    assert_eq!(backend.parent_of(inside.get().id()), Some(card.get().id()));
    assert_eq!(bounds(&ui, &card), Rect::new(0, 0, 400, 48), "28 + 2 x 10");
    assert_eq!(
        bounds(&ui, &inside),
        Rect::new(10, 10, 390, 38),
        "panel coordinates"
    );
}

#[test]
fn a_colour_panel_lays_its_children_out_where_it_is_placed() {
    let (_backend, _window, ui, _runtime) = setup();
    let chooser = Handle::<ColorPanel<u32>>::new();
    let _mounted = ui
        .mount(column().child(color_panel().bind(&chooser).fill(1)))
        .unwrap();
    assert_eq!(bounds(&ui, &chooser), Rect::new(0, 0, 400, 300));
}

#[test]
fn a_scroll_view_measures_its_rows_and_lays_them_out_when_placed() {
    let (_backend, _window, ui, _runtime) = setup();
    let view = Handle::<ScrollView<u32>>::new();
    let rows: std::rc::Rc<std::cell::RefCell<Vec<Button<u32>>>> = Default::default();
    let keep = std::rc::Rc::clone(&rows);
    let _mounted = ui
        .mount(
            column().child(
                build(|ui| ScrollView::new(ui, Rect::default()))
                    .then_with(move |view, _| {
                        let inner = Button::new(view.ui(), Rect::default(), "row")?;
                        view.add(inner.id(), Dip(40.0));
                        keep.borrow_mut().push(inner);
                        Ok(view)
                    })
                    .bind(&view),
            ),
        )
        .unwrap();
    assert_eq!(bounds(&ui, &view), Rect::new(0, 0, 400, 40));
    assert_eq!(ui.bounds(rows.borrow()[0].id()), Rect::new(0, 0, 400, 40));
}

#[test]
fn children_take_a_vec_of_layouts() {
    let (_backend, _window, ui, _runtime) = setup();
    let rows: Vec<crate::arrange::Layout<u32>> = (0..3).map(|_| row().child(button("x"))).collect();
    let last = Handle::<Button<u32>>::new();
    let _mounted = ui
        .mount(column().children(rows).child(button("last").bind(&last)))
        .unwrap();
    assert_eq!(bounds(&ui, &last).top, 3 * 28);
}
