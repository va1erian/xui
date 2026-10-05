//! `ListModel::icon` paints a Lucide icon in the leading slot of the rows that
//! have one, in the theme's text colour, at every DPI, and leaves rows without
//! one untouched.

use xui_canvas::snapshot::{Snapshot, render};
use xui_core::app::{App, Ui};
use xui_core::icon::{IconRef, Lucide};
use xui_core::image::Image;
use xui_core::prelude::{LayoutExt, column, list};
use xui_core::widget::ListModel;
use xui_core::{Dip, Theme};

/// Four rows; the odd ones carry an error icon.
struct Model;

impl ListModel for Model {
    fn rows(&self) -> usize {
        4
    }

    fn cell(&self, _row: usize, _column: usize) -> Option<&str> {
        Some("")
    }

    fn icon(&self, row: usize) -> Option<IconRef> {
        (row % 2 == 1).then(|| Lucide::CircleX.into())
    }
}

struct Demo;

impl App for Demo {
    type Msg = ();

    fn update(&mut self, _msg: (), _ui: &mut Ui<()>) {}
}

/// How many pixels of the `w` x `h` box at `x`, `y` differ from `background`.
fn marked(image: &Image, (x, y, w, h): (u32, u32, u32, u32), background: [u8; 4]) -> usize {
    let mut count = 0;
    for py in y..y + h {
        for px in x..x + w {
            if image.pixel(px, py) != Some(background) {
                count += 1;
            }
        }
    }
    count
}

fn check(theme: Theme, dpi: u32) {
    let image = render(
        Snapshot::new(Dip(200.0), Dip(100.0)).theme(theme).dpi(dpi),
        |ui| {
            let list = list().then(|list| {
                list.set_model(Model);
                list
            });
            ui.root(column().child(list.fill(1))).unwrap();
            Demo
        },
    )
    .expect("a snapshot");
    let scale = |dip: u32| dip * dpi / 96;
    let background = image
        .pixel(scale(150), scale(22 * 2 + 11))
        .expect("background pixel");
    // The 16 DIP icon slot sits after the 6 DIP inset, centred in the 22 DIP
    // row (inset vertically to stay clear of the separator on a row's top edge).
    let slot = |row: u32| (scale(6), scale(22 * row + 3), scale(16), scale(16));
    assert_eq!(
        marked(&image, slot(2), background),
        0,
        "a row without an icon stays blank in the slot ({theme:?}, {dpi} dpi)"
    );
    assert!(
        marked(&image, slot(3), background) > 20,
        "the icon paints in its slot ({theme:?}, {dpi} dpi)"
    );
}

#[test]
fn a_row_icon_paints_in_its_slot_light_and_dark_at_every_dpi() {
    for dpi in [96, 144, 192] {
        check(Theme::light(), dpi);
        check(Theme::dark(), dpi);
    }
}
