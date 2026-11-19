//! A body label beside buttons in a centred row must sit on the same centre
//! line as the buttons' own text: the ink of the label and of a button's label
//! are compared on the offscreen backend.

use std::cell::Cell;
use std::rc::Rc;

use xui_canvas::snapshot::{Snapshot, try_render};
use xui_core::app::{App, Ui};
use xui_core::geometry::Rect;
use xui_core::image::Image;
use xui_core::layout::Align;
use xui_core::prelude::{Handle, button, column, label, row};
use xui_core::widget::{Button, Label};
use xui_core::{Dip, Theme};

struct Host;

impl App for Host {
    type Msg = ();

    fn update(&mut self, _msg: (), _ui: &mut Ui<()>) {}
}

/// The vertical span `(top, bottom)` of the dark ink inside `area`, or `None`.
fn ink_rows(image: &Image, area: Rect) -> Option<(i32, i32)> {
    let dark = |x: i32, y: i32| {
        image
            .pixel(x as u32, y as u32)
            .is_some_and(|p| p[0] < 110 && p[1] < 110 && p[2] < 110)
    };
    let rows: Vec<i32> = (area.top..area.bottom)
        .filter(|&y| (area.left..area.right).any(|x| dark(x, y)))
        .collect();
    Some((*rows.first()?, *rows.last()? + 1))
}

/// `(label ink centre, button ink centre)` of a centred row, in pixels, as
/// twice the centre so a half pixel is visible.
fn centres(dpi: u32) -> (i32, i32) {
    let label_box = Rc::new(Cell::new(Rect::default()));
    let button_box = Rc::new(Cell::new(Rect::default()));
    let (label_out, button_out) = (Rc::clone(&label_box), Rc::clone(&button_box));
    let image = try_render(
        Snapshot::new(Dip(200.0), Dip(60.0))
            .theme(Theme::light())
            .dpi(dpi),
        move |ui| {
            let text = Handle::<Label<()>>::new();
            let action = Handle::<Button<()>>::new();
            ui.root(
                column().padding(16).child(
                    row()
                        .gap(8)
                        .align(Align::Center)
                        .children((button("2x").bind(&action), label("2x").bind(&text))),
                ),
            )?;
            label_out.set(ui.bounds(text.get().id()));
            button_out.set(ui.bounds(action.get().id()));
            Ok(Host)
        },
    )
    .expect("render");

    let (label_box, button_box) = (label_box.get(), button_box.get());
    assert!(
        label_box.height() > 0 && button_box.height() > 0,
        "laid out"
    );
    // Inside the button, away from its border; the label's own box is clear.
    let inner = button_box.shrink(4);
    let (bt, bb) = ink_rows(&image, inner).expect("button text ink");
    let (lt, lb) = ink_rows(&image, label_box).expect("label text ink");
    (lt + lb, bt + bb)
}

#[test]
fn a_body_label_sits_on_its_neighbour_buttons_centre_line() {
    for dpi in [96, 144] {
        let (label, button) = centres(dpi);
        assert!(
            (label - button).abs() <= 2,
            "at {dpi} dpi the label's ink centre is {} px, the button's {} px (doubled)",
            label,
            button
        );
    }
}
