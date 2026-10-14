//! A stroke is centred on the edge of its rectangle, so a widget that strokes
//! its own frame must not do it under a clip of that same rectangle: the clip
//! would cut the half of the stroke outside it (a two pixel soft frame becomes
//! one half-weight pixel, and none at all where a scrollbar covers the inner
//! half). These tests read the frame's pixels on all four sides, and a caret
//! drawn on a clip's edge.

use xui_canvas::snapshot::{Snapshot, render_with};
use xui_core::app::{App, Ui};
use xui_core::image::Image;
use xui_core::prelude::{LayoutExt, absolute, column, edit, label, scroll};
use xui_core::{Dip, Rect, Theme};

struct Host;

impl App for Host {
    type Msg = ();
    fn update(&mut self, _msg: (), _ui: &mut Ui<()>) {}
}

fn pixel(image: &Image, x: i32, y: i32) -> [u8; 4] {
    image
        .pixel(x as u32, y as u32)
        .unwrap_or_else(|| panic!("pixel ({x}, {y}) is inside the image"))
}

/// A scroll view that overflows (so it has a bar on its right) at `view` in a
/// 240 x 160 window.
fn scroll_view_frame(theme: Theme, view: Rect) -> Image {
    render_with(
        Snapshot::new(Dip(240.0), Dip(160.0)).theme(theme),
        move |ui| {
            let rows: Vec<_> = (1..=8)
                .map(|row| label(format!("Row {row}")).fixed(30))
                .collect();
            ui.root(absolute().child(scroll(column().children(rows)).at(
                view.left,
                view.top,
                view.width(),
                view.height(),
            )))?;
            Ok(Host)
        },
        |_| {},
    )
    .expect("the scroll view renders")
}

#[test]
fn a_scroll_view_keeps_its_two_pixel_frame_on_every_side() {
    let view = Rect::new(20, 20, 180, 120);
    let (mx, my) = ((view.left + view.right) / 2, (view.top + view.bottom) / 2);
    for theme in [Theme::light(), Theme::dark(), Theme::midnight()] {
        let image = scroll_view_frame(theme, view);
        // Each side's outer half pixel (just outside the view) against the
        // window two pixels further out, which no frame reaches. The right
        // side is where the scrollbar covers the frame's inner half, so the
        // outer one is all there is of its frame.
        let sides = [
            ("top", (mx, view.top - 1), (mx, view.top - 3)),
            ("bottom", (mx, view.bottom), (mx, view.bottom + 2)),
            ("left", (view.left - 1, my), (view.left - 3, my)),
            ("right", (view.right, my), (view.right + 2, my)),
        ];
        for (side, (fx, fy), (bx, by)) in sides {
            assert_ne!(
                pixel(&image, fx, fy),
                pixel(&image, bx, by),
                "the frame's outer half pixel on the {side} side is drawn"
            );
        }
        // The inner half pixel on the three sides the bar does not cover
        // differs from the outer one: both halves are there.
        let inside = [
            ("top", (mx, view.top), (mx, view.top - 1)),
            ("bottom", (mx, view.bottom - 1), (mx, view.bottom)),
            ("left", (view.left, my), (view.left - 1, my)),
        ];
        for (side, (ix, iy), (ox, oy)) in inside {
            assert_ne!(
                pixel(&image, ix, iy),
                pixel(&image, ox, oy),
                "the frame has an inner half pixel on the {side} side"
            );
        }
    }
}

/// An edit field at `field` in a 240 x 60 window; `click` focuses it with the
/// caret at the start of its text.
fn field(text: &str, field: Rect, click: bool) -> Image {
    let text = text.to_owned();
    render_with(
        Snapshot::new(Dip(240.0), Dip(60.0)).theme(Theme::light()),
        move |ui| {
            ui.root(absolute().child(edit().text(text).at(
                field.left,
                field.top,
                field.width(),
                field.height(),
            )))?;
            Ok(Host)
        },
        move |stage| {
            if click {
                stage.click(field.left + 2, field.top + field.height() / 2);
            }
        },
    )
    .expect("the field renders")
}

/// The x of the pixels in the middle row of `field`, to the right of its
/// border, that the focus changed: the caret's columns.
fn caret_columns(text: &str, area: Rect) -> Vec<u32> {
    let blurred = field(text, area, false);
    let focused = field(text, area, true);
    let row = (area.top + area.height() / 2) as u32;
    // Past the border, which the focus recolours, and clear of the text.
    let from = (area.left + 3) as u32;
    let to = (area.left + 12) as u32;
    (from..to)
        .filter(|&x| blurred.pixel(x, row) != focused.pixel(x, row))
        .collect()
}

#[test]
fn a_caret_before_the_first_character_is_drawn_whole() {
    let area = Rect::new(10, 10, 210, 38);
    // An empty field draws its caret outside any clip; a field whose text
    // starts with a space has its caret on the edge of the text clip, with no
    // glyph ink near it. Both put it at the start of the text.
    let empty = caret_columns("", area);
    let spaced = caret_columns(" x", area);
    assert!(
        empty.len() >= 2,
        "a caret one pixel wide at an integer x covers two columns: {empty:?}"
    );
    assert_eq!(
        spaced, empty,
        "the caret at the start of the text is not cut by the text's clip"
    );
}
