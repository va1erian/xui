//! Grids, alignment and caps.

use super::{bounds, setup};
use crate::arrange::{Align, Handle, LayoutExt, Track, button, column, grid, row};
use crate::geometry::Rect;
use crate::widget::Button;

type Buttons = Vec<Handle<Button<u32>>>;

fn handles(count: usize) -> Buttons {
    (0..count).map(|_| Handle::new()).collect()
}

#[test]
fn a_grid_sizes_auto_columns_to_their_widest_item_and_fills_the_rest() {
    let (_backend, _window, ui, _runtime) = setup();
    let h = handles(4);
    let _mounted = ui
        .mount(grid([Track::Auto, Track::Fill(1)]).gap(10).children((
            button("Name").bind(&h[0]).width(60),
            button("field").bind(&h[1]),
            button("Address").bind(&h[2]).width(90),
            button("field").bind(&h[3]),
        )))
        .unwrap();

    // Column 0 is as wide as the widest label (90); rows are 28 tall.
    assert_eq!(bounds(&ui, &h[0]), Rect::new(0, 0, 90, 28));
    assert_eq!(bounds(&ui, &h[1]), Rect::new(100, 0, 400, 28));
    assert_eq!(bounds(&ui, &h[2]), Rect::new(0, 38, 90, 66));
    assert_eq!(bounds(&ui, &h[3]), Rect::new(100, 38, 400, 66));
}

#[test]
fn a_span_covers_columns_and_a_fill_item_makes_its_row_fill() {
    let (_backend, _window, ui, _runtime) = setup();
    let h = handles(4);
    let _mounted = ui
        .mount(
            grid([Track::Fill(1), Track::Fill(1), Track::Fill(1)]).children((
                button("1").bind(&h[0]),
                button("2").bind(&h[1]),
                button("wide").bind(&h[2]).span(2),
                button("tall").bind(&h[3]).span(3).fill(1),
            )),
        )
        .unwrap();

    assert_eq!(bounds(&ui, &h[0]), Rect::new(0, 0, 133, 28));
    assert_eq!(bounds(&ui, &h[1]), Rect::new(133, 0, 266, 28));
    // One column is left in the first row, so the span starts the next.
    assert_eq!(bounds(&ui, &h[2]), Rect::new(0, 28, 266, 56));
    assert_eq!(bounds(&ui, &h[3]), Rect::new(0, 56, 400, 300));
}

#[test]
fn an_aligned_item_keeps_its_natural_cross_size() {
    let (_backend, _window, ui, _runtime) = setup();
    let h = handles(3);
    let _mounted = ui
        .mount(row().align(Align::Center).children((
            button("a").bind(&h[0]).width(40),
            button("b").bind(&h[1]).width(40).align(Align::End),
            button("c").bind(&h[2]).width(40).align(Align::Stretch),
        )))
        .unwrap();

    assert_eq!(bounds(&ui, &h[0]), Rect::new(0, 136, 40, 164));
    assert_eq!(bounds(&ui, &h[1]), Rect::new(40, 272, 80, 300));
    assert_eq!(bounds(&ui, &h[2]), Rect::new(80, 0, 120, 300));
}

#[test]
fn justify_centres_packed_items_and_a_cap_narrows_a_stretched_one() {
    let (_backend, _window, ui, _runtime) = setup();
    let h = handles(3);
    let _mounted = ui
        .mount(column().children((
            row().justify(Align::Center).children((
                button("ok").bind(&h[0]).width(100),
                button("cancel").bind(&h[1]).width(100),
            )),
            button("capped").bind(&h[2]).max_width(250),
        )))
        .unwrap();

    assert_eq!(bounds(&ui, &h[0]), Rect::new(100, 0, 200, 28));
    assert_eq!(bounds(&ui, &h[1]), Rect::new(200, 0, 300, 28));
    assert_eq!(bounds(&ui, &h[2]), Rect::new(0, 28, 250, 56));
}
