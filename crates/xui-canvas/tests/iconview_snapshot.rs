//! Headless snapshots of [`IconView`] in every icon size, light and dark, with
//! three-line tiles, a focused and selected item and a name that must
//! ellipsise.
//!
//! Writes `target/snapshots/iconview-{small,medium,large}-{light,dark}.png` for
//! eyeballing and asserts that the icon slot and text block painted, that the
//! selection differs from an ordinary tile and that dark differs from light.

use std::cell::Cell;
use std::rc::Rc;

use xui_canvas::snapshot::{Snapshot, render_with};
use xui_core::app::{App, Ui};
use xui_core::backend::{Event, WidgetId};
use xui_core::icon::{IconRef, Lucide};
use xui_core::image::Image;
use xui_core::widget::{IconModel, IconSize, IconView};
use xui_core::{Dip, Rect, Theme};

const NAMES: [&str; 6] = [
    "Quarterly report with a very long name",
    "Photo",
    "Notes",
    "Music library",
    "Project files",
    "Archive",
];
const KINDS: [&str; 6] = [
    "Text Document",
    "Image",
    "Text Document",
    "Audio",
    "Folder",
    "Compressed",
];
const SIZES: [&str; 6] = ["12 KB", "2.4 MB", "1 KB", "48 MB", "", "6.1 MB"];

struct Model;

impl IconModel for Model {
    fn items(&self) -> usize {
        NAMES.len()
    }

    fn icon(&self, item: usize) -> Option<IconRef> {
        Some(if item.is_multiple_of(2) {
            Lucide::FolderOpen.into()
        } else {
            Lucide::File.into()
        })
    }

    fn line(&self, item: usize, line: usize) -> Option<&str> {
        match line {
            0 => Some(NAMES[item % NAMES.len()]),
            1 => Some(KINDS[item % KINDS.len()]),
            2 => Some(SIZES[item % SIZES.len()]),
            _ => None,
        }
    }
}

struct Demo(#[allow(dead_code)] IconView<()>);

impl App for Demo {
    type Msg = ();

    fn update(&mut self, _msg: (), _ui: &mut Ui<()>) {}
}

/// The design geometry the widget uses at 96 DPI, mirrored here because the
/// layout module is crate-private (see `iconview/layout.rs`).
struct Geom {
    icon: u32,
    pad_x: u32,
    icon_gap: u32,
    tile_w: u32,
    tile_h: u32,
    line_h: u32,
    lines: u32,
    gap: u32,
}

fn geom(size: IconSize) -> Geom {
    match size {
        IconSize::Small => Geom {
            icon: 16,
            pad_x: 5,
            icon_gap: 6,
            tile_w: 180,
            tile_h: 22,
            line_h: 14,
            lines: 1,
            gap: 4,
        },
        IconSize::Medium => Geom {
            icon: 32,
            pad_x: 6,
            icon_gap: 6,
            tile_w: 190,
            tile_h: 50,
            line_h: 15,
            lines: 3,
            gap: 4,
        },
        IconSize::Large => Geom {
            icon: 48,
            pad_x: 7,
            icon_gap: 8,
            tile_w: 210,
            tile_h: 66,
            line_h: 16,
            lines: 3,
            gap: 4,
        },
    }
}

/// Renders one size and theme, focusing the view and selecting item 1.
fn shoot(size: IconSize, label: &str, theme: Theme) -> Image {
    let id = Rc::new(Cell::new(WidgetId::NONE));
    let id_for_build = Rc::clone(&id);
    render_with(
        Snapshot::new(Dip(700.0), Dip(260.0))
            .theme(theme)
            .title(format!("iconview {label}")),
        move |ui| {
            let view = IconView::with_model(ui, Rect::new(0, 0, 700, 260), Model).expect("view");
            view.set_icon_size(size);
            view.select(Some(1));
            id_for_build.set(view.id());
            Ok(Demo(view))
        },
        move |stage| {
            let id = id.get();
            if !id.is_none() {
                stage.ui().focus(id);
                stage.inject(Event::SetFocus);
            }
        },
    )
    .expect("a snapshot")
}

fn save(name: &str, image: &Image) {
    let dir = std::path::Path::new("target/snapshots");
    let _ = std::fs::create_dir_all(dir);
    image
        .save_png(dir.join(name))
        .expect("save the snapshot png");
}

/// Pixels in the `(x, y, w, h)` box that differ from `background`.
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

/// Whether any pixel in the box is exactly `color` (the selection fill).
fn has_color(image: &Image, (x, y, w, h): (u32, u32, u32, u32), color: xui_core::Color) -> bool {
    let want = [color.r, color.g, color.b, 255];
    (y..y + h).any(|py| (x..x + w).any(|px| image.pixel(px, py) == Some(want)))
}

fn differing(a: &Image, b: &Image) -> usize {
    let mut count = 0;
    for y in 0..a.height().min(b.height()) {
        for x in 0..a.width().min(b.width()) {
            if a.pixel(x, y).map(|p| [p[0], p[1], p[2]])
                != b.pixel(x, y).map(|p| [p[0], p[1], p[2]])
            {
                count += 1;
            }
        }
    }
    count
}

fn check(size: IconSize, label: &str) {
    let light = shoot(size, label, Theme::light());
    let dark = shoot(size, label, Theme::dark());
    save(&format!("iconview-{label}-light.png"), &light);
    save(&format!("iconview-{label}-dark.png"), &dark);

    let geom = geom(size);
    // A patch below the tiles is never drawn on, so it gives each theme's own
    // background colour.
    for (image, theme) in [(&light, Theme::light()), (&dark, Theme::dark())] {
        let background = image
            .pixel(690, 250)
            .map(|p| [p[0], p[1], p[2], p[3]])
            .expect("background pixel");

        let icon_slot = (
            geom.pad_x,
            (geom.tile_h - geom.icon) / 2,
            geom.icon,
            geom.icon,
        );
        assert!(
            marked(image, icon_slot, background) > 10,
            "the icon paints in its slot ({label}, {theme:?})"
        );

        let text_top = (geom.tile_h - geom.lines * geom.line_h) / 2;
        let text_left = geom.pad_x + geom.icon + geom.icon_gap;
        let text = (
            text_left,
            text_top,
            geom.tile_w - geom.pad_x - text_left,
            geom.lines * geom.line_h,
        );
        assert!(
            marked(image, text, background) > 20,
            "the text block paints ({label}, {theme:?})"
        );

        // The ellipsised long name never bleeds into the gap after its tile.
        assert_eq!(
            marked(image, (geom.tile_w, 0, geom.gap, geom.tile_h), background),
            0,
            "the long name is clipped to its tile ({label}, {theme:?})"
        );

        // Item 1 is selected and focused: its text block carries the accent
        // fill, which an ordinary tile's does not.
        let selected_text = (geom.tile_w + geom.gap + text_left, text_top, text.2, text.3);
        assert!(
            has_color(image, selected_text, theme.accent),
            "the selected tile is filled with the accent ({label}, {theme:?})"
        );
        assert!(
            !has_color(image, text, theme.accent),
            "an ordinary tile carries no accent fill ({label}, {theme:?})"
        );
    }

    assert!(
        differing(&light, &dark) > 500,
        "light and dark differ ({label})"
    );
}

#[test]
fn icon_view_snapshots_every_size_light_and_dark() {
    check(IconSize::Small, "small");
    check(IconSize::Medium, "medium");
    check(IconSize::Large, "large");
}
