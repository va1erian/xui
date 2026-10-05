//! `ToggleButton::icon` paints a Lucide icon centred in an icon-only button and
//! before the label of a labelled one, in the label's colour: the text colour
//! when unchecked and the on-accent colour when checked, light and dark, at
//! every DPI.
//!
//! The PNGs (`target/snapshots/togglebutton-icons-{light,dark}.png`) are the
//! visual check.

use std::path::PathBuf;

use xui_canvas::snapshot::{Snapshot, render};
use xui_core::app::{App, Ui};
use xui_core::icon::Lucide;
use xui_core::image::Image;
use xui_core::prelude::{LayoutExt, absolute, toggle_button};
use xui_core::{Color, Dip, Theme};

struct Demo;

impl App for Demo {
    type Msg = ();

    fn update(&mut self, _msg: (), _ui: &mut Ui<()>) {}
}

/// The DIP boxes of the buttons: icon-only unchecked, icon-only checked,
/// icon-and-label unchecked, icon-and-label checked.
const BOXES: [(i32, i32, i32, i32); 4] = [
    (8, 8, 28, 28),
    (44, 8, 28, 28),
    (8, 44, 96, 28),
    (112, 44, 96, 28),
];

/// Renders the four buttons in `theme` at `dpi`.
fn shot(theme: Theme, dpi: u32) -> Image {
    render(
        Snapshot::new(Dip(216.0), Dip(80.0)).theme(theme).dpi(dpi),
        |ui| {
            let make = |(x, y, w, h): (i32, i32, i32, i32), text: &str, checked: bool| {
                toggle_button(text)
                    .icon(Lucide::Bold)
                    .checked(checked)
                    .at(x, y, w, h)
            };
            ui.root(absolute().children((
                make(BOXES[0], "", false),
                make(BOXES[1], "", true),
                make(BOXES[2], "Bold", false),
                make(BOXES[3], "Bold", true),
            )))
            .expect("the toggle buttons");
            Demo
        },
    )
    .expect("a snapshot")
}

/// The device-pixel `(x, y, w, h)` of a DIP box.
fn scaled(dpi: u32, (x, y, w, h): (i32, i32, i32, i32)) -> (u32, u32, u32, u32) {
    let s = |v: i32| (v * dpi as i32 / 96) as u32;
    (s(x), s(y), s(w), s(h))
}

/// The distance from `color` of the pixel in the box that comes closest to it.
fn closest(image: &Image, (x, y, w, h): (u32, u32, u32, u32), color: Color) -> u32 {
    let mut best = u32::MAX;
    for py in y..y + h {
        for px in x..x + w {
            let pixel = image.pixel(px, py).expect("pixel in bounds");
            let distance = [
                (pixel[0], color.r),
                (pixel[1], color.g),
                (pixel[2], color.b),
            ]
            .iter()
            .map(|&(a, b)| i32::from(a).abs_diff(i32::from(b)))
            .sum();
            best = best.min(distance);
        }
    }
    best
}

/// How many pixels in the box differ from the face colour at its top-left.
fn marked(image: &Image, (x, y, w, h): (u32, u32, u32, u32)) -> usize {
    let face = image.pixel(x, y).expect("face pixel");
    let mut count = 0;
    for py in y..y + h {
        for px in x..x + w {
            if image.pixel(px, py) != Some(face) {
                count += 1;
            }
        }
    }
    count
}

fn check(theme: Theme, dpi: u32) -> Image {
    let image = shot(theme, dpi);
    // The icon-only buttons centre their icon: the middle 12 DIP square of the
    // 28 DIP face carries its strokes.
    for (index, checked) in [(0, false), (1, true)] {
        let (x, y, _, _) = BOXES[index];
        let middle = scaled(dpi, (x + 8, y + 8, 12, 12));
        assert!(
            marked(&image, middle) > 10,
            "the icon paints in an icon-only button ({theme:?}, {dpi} dpi, checked {checked})"
        );
        let (ink, other) = if checked {
            (theme.text_on_accent, theme.text)
        } else {
            (theme.text, theme.text_on_accent)
        };
        assert!(
            closest(&image, middle, ink) < closest(&image, middle, other),
            "the icon takes the label colour ({theme:?}, {dpi} dpi, checked {checked})"
        );
    }
    // A labelled button puts the icon at its leading edge, before the label:
    // the strip after the 6 px gap holds the icon's strokes.
    for index in [2, 3] {
        let (x, y, _, h) = BOXES[index];
        let lead = scaled(dpi, (x + 8, y + 6, 14, h - 12));
        assert!(
            marked(&image, lead) > 10,
            "the icon paints before the label ({theme:?}, {dpi} dpi)"
        );
    }
    image
}

#[test]
fn icons_paint_light_and_dark_at_every_dpi() {
    for dpi in [96, 144, 192] {
        check(Theme::light(), dpi);
        check(Theme::dark(), dpi);
    }
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/snapshots");
    std::fs::create_dir_all(&dir).expect("snapshot dir");
    for (name, theme) in [("light", Theme::light()), ("dark", Theme::dark())] {
        check(theme, 192)
            .save_png(dir.join(format!("togglebutton-icons-{name}.png")))
            .expect("save snapshot");
    }
}
