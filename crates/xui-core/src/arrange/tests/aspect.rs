//! Aspect ratios in a mounted layout: a named width keeps the ratio through a
//! resize, and a filling entry takes it from the space it gets.

use super::{bounds, setup};
use crate::arrange::{Handle, LayoutExt, button, column, label, row};
use crate::geometry::Rect;
use crate::widget::Button;

#[test]
fn an_aspect_entry_keeps_its_ratio_through_a_resize() {
    let (backend, window, ui, _runtime) = setup();
    let preview = Handle::<Button<u32>>::new();
    let _mounted = ui
        .mount(column().child(row().child(button("preview").bind(&preview).width(160).aspect(2.0))))
        .unwrap();
    assert_eq!(bounds(&ui, &preview), Rect::new(0, 0, 160, 80));

    backend.resize_window(window, 500, 200);
    assert_eq!(
        bounds(&ui, &preview),
        Rect::new(0, 0, 160, 80),
        "a named width keeps the ratio"
    );
}

#[test]
fn a_filling_aspect_entry_takes_the_ratio_from_the_space_it_gets() {
    let (_backend, _window, ui, _runtime) = setup();
    let canvas = Handle::<Button<u32>>::new();
    let _mounted = ui
        .mount(column().padding(8).gap(8).children((
            label("preview").fixed(20),
            button("canvas").bind(&canvas).aspect(16.0 / 9.0).fill(1),
        )))
        .unwrap();
    assert_eq!(
        bounds(&ui, &canvas),
        Rect::new(8, 36, 392, 252),
        "384 wide at 16:9 is 216 tall"
    );
}
