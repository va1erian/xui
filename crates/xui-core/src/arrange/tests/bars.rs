//! Toolbars, top bars, menu bars, item views, splits and flowing text in
//! layouts: each measures from its content.

use super::{bounds, setup};
use crate::arrange::{
    Handle, LayoutExt, button, column, flow_text, grid_view, icon_view, material_status_bar,
    menu_bar, row, split, toolbar, top_bar,
};
use crate::geometry::Rect;
use crate::icon::Lucide;
use crate::widget::{
    Button, FlowText, GridView, IconView, MaterialStatusBar, Menu, MenuId, Run, Toolbar, TopBar,
    TopBarId,
};

#[test]
fn bars_are_as_wide_as_their_items() {
    let (_backend, _window, ui, _runtime) = setup();
    let tools = Handle::<Toolbar<u32>>::new();
    let band = Handle::<TopBar<u32>>::new();
    let menu = Handle::<Menu<u32>>::new();
    let _mounted = ui
        .mount(
            column().children((
                row().child(
                    toolbar()
                        .item(Lucide::Save, "Save")
                        .separator()
                        .item(Lucide::Copy, "Copy")
                        .on_click(|index| index as u32)
                        .bind(&tools),
                ),
                row().child(
                    top_bar()
                        .then(|bar| bar.icon(TopBarId::new(1), Lucide::Menu))
                        .bind(&band),
                ),
                row().child(
                    menu_bar(|menus| {
                        menus.submenu(MenuId::new(1), "File", |file| {
                            file.item(MenuId::new(2), "Open");
                        });
                    })
                    .bind(&menu),
                ),
            )),
        )
        .unwrap();
    let strip = bounds(&ui, &tools);
    // Two square 32-dip buttons and a separator with its margins.
    assert_eq!((strip.width(), strip.height()), (32 + 9 + 32, 32));
    assert_eq!(
        bounds(&ui, &band).size(),
        crate::geometry::Size::new(36, 36)
    );
    let bar = ui.bounds(menu.get().id().unwrap());
    assert!(bar.width() > 0 && bar.height() == 28, "{bar:?}");
}

#[test]
fn item_views_and_status_bars_have_natural_sizes() {
    let (_backend, _window, ui, _runtime) = setup();
    let (grid, icons, status) = (
        Handle::<GridView<u32>>::new(),
        Handle::<IconView<u32>>::new(),
        Handle::<MaterialStatusBar<u32>>::new(),
    );
    let _mounted = ui
        .mount(column().children((
            row().children((
                grid_view(&["a", "b"]).on_select(|i| i as u32).bind(&grid),
                icon_view(&["c"]).on_activate(|i| i as u32).bind(&icons),
            )),
            material_status_bar(&["Ready"]).bind(&status),
        )))
        .unwrap();
    assert_eq!(
        bounds(&ui, &grid).height(),
        240,
        "the taller of 240 and 240"
    );
    assert_eq!(bounds(&ui, &status).height(), 24);
}

#[test]
fn a_split_lays_its_two_layouts_out_in_its_panes() {
    let (_backend, _window, ui, _runtime) = setup();
    let (left, right) = (Handle::<Button<u32>>::new(), Handle::<Button<u32>>::new());
    let _mounted = ui
        .mount(
            column().child(
                split(
                    column().child(button("left").bind(&left).fill(1)),
                    column().child(button("right").bind(&right).fill(1)),
                )
                .position(100)
                .fill(1),
            ),
        )
        .unwrap();
    // Pane-local coordinates: each button fills its pane's panel.
    assert_eq!(bounds(&ui, &left), Rect::new(0, 0, 100, 300));
    assert_eq!(bounds(&ui, &right).height(), 300);
    assert_eq!(bounds(&ui, &right).width(), 400 - 100 - 5);
}

#[test]
fn flowing_text_is_as_tall_as_its_lines_at_the_width_it_gets() {
    let (_backend, _window, ui, _runtime) = setup();
    let (narrow, wide) = (
        Handle::<FlowText<u32>>::new(),
        Handle::<FlowText<u32>>::new(),
    );
    let words = "many words that will need more than one line at a narrow width";
    let _mounted = ui
        .mount(
            row().children((
                column()
                    .child(flow_text().run(Run::normal(words)).bind(&narrow))
                    .width(80),
                column()
                    .child(flow_text().run(Run::normal(words)).bind(&wide))
                    .fill(1),
            )),
        )
        .unwrap();
    assert!(bounds(&ui, &narrow).height() > bounds(&ui, &wide).height());
}
