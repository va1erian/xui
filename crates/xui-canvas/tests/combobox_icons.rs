//! `ComboBox::item_icon` paints a Lucide icon before an item's text, in the
//! open list and in the closed box for the selected item, at every DPI, and
//! leaves items without one untouched.

use xui_canvas::snapshot::{Snapshot, render_with};
use xui_core::app::{App, Ui};
use xui_core::icon::Lucide;
use xui_core::image::Image;
use xui_core::prelude::{LayoutExt, absolute, combo_box};
use xui_core::{Color, Dip, Theme};

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

/// The distance from `color` of the painted pixel (one that is not
/// `background`) in the box that comes closest to it: small when the icon is
/// drawn in `color`, large when it is drawn in another, `u32::MAX` when nothing
/// is painted at all.
fn closest(
    image: &Image,
    (x, y, w, h): (u32, u32, u32, u32),
    background: [u8; 4],
    color: Color,
) -> u32 {
    let mut best = u32::MAX;
    for py in y..y + h {
        for px in x..x + w {
            let pixel = image.pixel(px, py).expect("pixel in bounds");
            if pixel == background {
                continue;
            }
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

/// Renders a 28 DIP-high combo over three items (icons on 0 and 2; item 1 is a
/// blank label, so its text cannot mark the icon slot) with
/// `selected` chosen, optionally with the list open.
fn shot(theme: Theme, dpi: u32, selected: usize, open: bool, enabled: bool) -> Image {
    let scale = move |dip: i32| dip * dpi as i32 / 96;
    render_with(
        Snapshot::new(Dip(200.0), Dip(140.0)).theme(theme).dpi(dpi),
        move |ui| {
            let combo = combo_box(&["one", " ", "three"]).then(move |combo| {
                let combo = combo
                    .item_icon(0, Lucide::CircleDot)
                    .item_icon(2, Lucide::TriangleAlert);
                combo.select(selected);
                combo.set_enabled(enabled);
                combo
            });
            ui.root(absolute().child(combo.at(0, 0, 160, 28)))?;
            Ok(Demo)
        },
        move |stage| {
            if open {
                stage.click(scale(20), scale(14));
            }
        },
    )
    .expect("a snapshot")
}

fn scaled(dpi: u32, (x, y, w, h): (u32, u32, u32, u32)) -> (u32, u32, u32, u32) {
    let s = |v: u32| v * dpi / 96;
    (s(x), s(y), s(w), s(h))
}

fn check(theme: Theme, dpi: u32) {
    // Closed: the field shows the selected item's icon in the 16 DIP slot after
    // its 4 DIP inset (centred in the 28 DIP field), and nothing for a
    // selected item without one.
    let slot = scaled(dpi, (4, 6, 16, 16));
    let with_icon = shot(theme, dpi, 0, false, true);
    let background = with_icon
        .pixel(dpi * 100 / 96, dpi * 14 / 96)
        .expect("field background");
    assert!(
        marked(&with_icon, slot, background) > 20,
        "the selected item's icon paints in the closed box ({theme:?}, {dpi} dpi)"
    );
    let without = shot(theme, dpi, 1, false, true);
    assert_eq!(
        marked(&without, slot, background),
        0,
        "a selected item without an icon leaves the slot blank ({theme:?}, {dpi} dpi)"
    );

    // The icon takes the text colour, and the disabled colour when disabled.
    let disabled = shot(theme, dpi, 0, false, false);
    assert!(
        marked(&disabled, slot, background) > 20,
        "a disabled combo still shows the icon ({theme:?}, {dpi} dpi)"
    );
    assert!(
        closest(&disabled, slot, background, theme.text_disabled)
            < closest(&disabled, slot, background, theme.text),
        "a disabled combo draws the icon dimmed ({theme:?}, {dpi} dpi)"
    );
    assert!(
        closest(&with_icon, slot, background, theme.text)
            < closest(&with_icon, slot, background, theme.text_disabled),
        "an enabled combo draws the icon in the text colour ({theme:?}, {dpi} dpi)"
    );

    // Open: rows are 24 DIP high, directly below the field; row 1 has no icon,
    // row 2 has one. The hovered row is row 0 (under the click), so the
    // background is sampled from row 1's own band.
    let open = shot(theme, dpi, 0, true, true);
    let list_background = open
        .pixel(dpi * 120 / 96, dpi * (28 + 24 + 12) / 96)
        .expect("list background");
    assert_eq!(
        marked(
            &open,
            scaled(dpi, (4, 28 + 24 + 4, 16, 16)),
            list_background
        ),
        0,
        "an item without an icon leaves its slot blank in the list ({theme:?}, {dpi} dpi)"
    );
    // Row 0 is the highlighted row, so its icon takes the on-accent colour.
    let hot = scaled(dpi, (4, 28 + 4, 16, 16));
    let hot_background = open
        .pixel(dpi * 120 / 96, dpi * (28 + 12) / 96)
        .expect("highlighted row background");
    assert!(
        marked(&open, hot, hot_background) > 20,
        "the highlighted row's icon paints ({theme:?}, {dpi} dpi)"
    );
    assert!(
        closest(&open, hot, hot_background, theme.text_on_accent)
            < closest(&open, hot, hot_background, theme.text),
        "the highlighted row's icon is on-accent ({theme:?}, {dpi} dpi)"
    );
    assert!(
        marked(
            &open,
            scaled(dpi, (4, 28 + 48 + 4, 16, 16)),
            list_background
        ) > 20,
        "the icon paints in the list ({theme:?}, {dpi} dpi)"
    );
}

#[test]
fn item_icons_paint_light_and_dark_at_every_dpi() {
    for dpi in [96, 144, 192] {
        check(Theme::light(), dpi);
        check(Theme::dark(), dpi);
    }
}
