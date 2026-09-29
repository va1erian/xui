//! `MenuScope::icon` paints a Lucide icon in the icon column of a popup, in the
//! entry's text colour (dimmed when disabled), at every DPI, and leaves
//! entries without one untouched.

use xui_canvas::snapshot::{Snapshot, render};
use xui_core::app::{App, Ui};
use xui_core::icon::Lucide;
use xui_core::image::Image;
use xui_core::widget::{Menu, MenuId};
use xui_core::{Dip, Theme};

struct Demo(#[allow(dead_code)] Menu<()>);

impl App for Demo {
    type Msg = ();

    fn update(&mut self, _msg: (), _ui: &mut Ui<()>) {}
}

/// The largest per-channel-sum distance of any pixel in the box from
/// `background`, and how many pixels differ at all.
fn ink(image: &Image, (x, y, w, h): (u32, u32, u32, u32), background: [u8; 4]) -> (u32, usize) {
    let (mut strongest, mut count) = (0, 0);
    for py in y..y + h {
        for px in x..x + w {
            let pixel = image.pixel(px, py).expect("pixel in bounds");
            if pixel != background {
                count += 1;
                let distance = (0..3)
                    .map(|c| (i32::from(pixel[c]) - i32::from(background[c])).unsigned_abs())
                    .sum();
                strongest = strongest.max(distance);
            }
        }
    }
    (strongest, count)
}

/// A context menu at (10, 10) DIP: icons on rows 0 and 2, a blank row 1, and
/// row 2 disabled when `disable_third`.
fn shot(theme: Theme, dpi: u32, disable_third: bool) -> Image {
    let scale = move |dip: i32| dip * dpi as i32 / 96;
    render(
        Snapshot::new(Dip(240.0), Dip(150.0)).theme(theme).dpi(dpi),
        move |ui| {
            let menu = Menu::context(ui).build(|m| {
                m.item(MenuId::new(1), "Copy").icon(Lucide::Copy);
                m.item(MenuId::new(2), " ");
                m.item(MenuId::new(3), "Delete").icon(Lucide::CircleX);
            });
            menu.set_enabled(MenuId::new(3), !disable_third);
            menu.show_context(scale(10), scale(10));
            Demo(menu)
        },
    )
    .expect("a snapshot")
}

fn slot(dpi: u32, row: u32) -> (u32, u32, u32, u32) {
    // Origin 10, 5 DIP pad, 24 DIP rows; a 16 DIP icon centred in a 22 DIP
    // column (3 DIP in) and in the row (4 DIP down).
    let s = |v: u32| v * dpi / 96;
    (s(10 + 3), s(10 + 5 + 24 * row + 4), s(16), s(16))
}

fn check(theme: Theme, dpi: u32) {
    let enabled = shot(theme, dpi, false);
    let s = |v: u32| v * dpi / 96;
    let background = enabled
        .pixel(s(10 + 120), s(10 + 5 + 24 + 12))
        .expect("popup background");

    let (strongest, count) = ink(&enabled, slot(dpi, 0), background);
    assert!(
        count > 20,
        "the icon paints in its slot ({theme:?}, {dpi} dpi)"
    );
    assert!(strongest > 0);
    assert_eq!(
        ink(&enabled, slot(dpi, 1), background).1,
        0,
        "an entry without an icon leaves the column blank ({theme:?}, {dpi} dpi)"
    );

    let disabled = shot(theme, dpi, true);
    let (enabled_ink, _) = ink(&enabled, slot(dpi, 2), background);
    let (disabled_ink, disabled_count) = ink(&disabled, slot(dpi, 2), background);
    assert!(disabled_count > 20, "a disabled entry still shows its icon");
    assert!(
        disabled_ink < enabled_ink,
        "a disabled icon is dimmed: {disabled_ink} vs {enabled_ink} ({theme:?}, {dpi} dpi)"
    );
}

#[test]
fn menu_icons_paint_light_and_dark_at_every_dpi() {
    for dpi in [96, 144, 192] {
        check(Theme::light(), dpi);
        check(Theme::dark(), dpi);
    }
}
