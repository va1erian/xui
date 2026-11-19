//! Where a body label's text sits in its box, on the offscreen backend.
//!
//! A single-line body label paints vertically centred in its box, whatever
//! the box height: at its natural height that puts it on the centre line of a
//! button beside it in a centred row, and in a taller box (stretched by a row,
//! or given a height) it stays centred. Multi-line text starts at the top, as
//! before.

use std::cell::Cell;
use std::rc::Rc;

use xui_canvas::Surface;
use xui_canvas::snapshot::{Snapshot, try_render};
use xui_core::app::{App, Ui};
use xui_core::geometry::Rect;
use xui_core::image::Image;
use xui_core::layout::Align;
use xui_core::prelude::{Handle, LayoutExt, button, column, label, row};
use xui_core::widget::{Button, Label};
use xui_core::{Canvas, Dip, TextStyle, Theme};

struct Host;

impl App for Host {
    type Msg = ();

    fn update(&mut self, _msg: (), _ui: &mut Ui<()>) {}
}

/// The vertical span `(top, bottom)` of the dark ink inside `area`, or `None`.
fn ink_rows(pixel: impl Fn(u32, u32) -> Option<[u8; 4]>, area: Rect) -> Option<(i32, i32)> {
    let dark = |x: i32, y: i32| {
        pixel(x as u32, y as u32).is_some_and(|p| p[0] < 110 && p[1] < 110 && p[2] < 110)
    };
    let rows: Vec<i32> = (area.top..area.bottom)
        .filter(|&y| (area.left..area.right).any(|x| dark(x, y)))
        .collect();
    Some((*rows.first()?, *rows.last()? + 1))
}

/// Twice the vertical centre of the ink in `area`, so a half pixel shows.
fn ink_centre2(image: &Image, area: Rect) -> i32 {
    let (top, bottom) = ink_rows(|x, y| image.pixel(x, y), area).expect("ink");
    top + bottom
}

/// Renders `build`'s root on the light theme and returns the image with the
/// device rectangles of the label and of the button it holds.
fn render(
    dpi: u32,
    build: impl Fn(&Handle<Label<()>>, &Handle<Button<()>>) -> xui_core::prelude::Layout<()> + 'static,
) -> (Image, Rect, Rect) {
    let label_box = Rc::new(Cell::new(Rect::default()));
    let button_box = Rc::new(Cell::new(Rect::default()));
    let (label_out, button_out) = (Rc::clone(&label_box), Rc::clone(&button_box));
    let image = try_render(
        Snapshot::new(Dip(240.0), Dip(200.0))
            .theme(Theme::light())
            .dpi(dpi),
        move |ui| {
            let text = Handle::<Label<()>>::new();
            let action = Handle::<Button<()>>::new();
            ui.root(build(&text, &action))?;
            label_out.set(ui.bounds(text.get().id()));
            button_out.set(ui.bounds(action.get().id()));
            Ok(Host)
        },
    )
    .expect("render");
    (image, label_box.get(), button_box.get())
}

#[test]
fn a_natural_height_label_sits_on_its_neighbour_buttons_centre_line() {
    for dpi in [96, 144] {
        let (image, label_box, button_box) = render(dpi, |text, action| {
            column().padding(16).child(
                row()
                    .gap(8)
                    .align(Align::Center)
                    .children((button("2x").bind(action), label("2x").bind(text))),
            )
        });
        assert!(
            label_box.height() > 0 && button_box.height() > 0,
            "laid out"
        );
        let label = ink_centre2(&image, label_box);
        let button = ink_centre2(&image, button_box.shrink(4));
        assert!(
            (label - button).abs() <= 2,
            "at {dpi} dpi the label's ink centre is {label} and the button's {button} (doubled)"
        );
    }
}

#[test]
fn a_label_stretched_by_its_row_is_centred_in_the_taller_box() {
    // A default row stretches the label to the height of the tall button.
    let (image, label_box, button_box) = render(96, |text, action| {
        column().padding(16).child(
            row()
                .gap(8)
                .children((button("2x").bind(action).height(64), label("2x").bind(text))),
        )
    });
    assert!(label_box.height() >= 60, "stretched: {label_box:?}");
    let centre = ink_centre2(&image, label_box);
    let middle = label_box.top + label_box.bottom;
    assert!(
        (centre - middle).abs() <= 3,
        "ink centre {centre} vs box centre {middle} (doubled), button {button_box:?}"
    );
}

#[test]
fn a_label_with_an_explicit_height_is_centred_in_it() {
    let (image, label_box, _) = render(96, |text, action| {
        column()
            .padding(16)
            .children((label("2x").bind(text).height(60), button("x").bind(action)))
    });
    assert_eq!(label_box.height(), 60);
    let centre = ink_centre2(&image, label_box);
    let middle = label_box.top + label_box.bottom;
    assert!((centre - middle).abs() <= 3, "ink {centre} vs box {middle}");
}

#[test]
fn a_multi_line_label_still_starts_at_the_top_of_its_box() {
    let (image, label_box, _) = render(96, |text, action| {
        column().padding(16).children((
            label("Ag\nAg").bind(text).height(80),
            button("x").bind(action),
        ))
    });
    assert_eq!(label_box.height(), 80);

    // The same text drawn directly with the default (top) text style.
    let theme = Theme::light();
    let mut surface = Surface::new(240, 200);
    surface.fill(theme.background);
    surface.with_canvas(label_box, |canvas| {
        let area = canvas.bounds();
        canvas.draw_text("Ag\nAg", area, &TextStyle::new(theme.text, Dip(12.0)));
    });
    let reference = surface.to_image();

    assert_eq!(
        ink_rows(|x, y| image.pixel(x, y), label_box),
        ink_rows(|x, y| reference.pixel(x, y), label_box),
        "a multi-line body label paints top-aligned, as it always did"
    );
    let (top, _) = ink_rows(|x, y| image.pixel(x, y), label_box).expect("ink");
    assert!(top - label_box.top <= 4, "ink starts at the top of the box");
}
