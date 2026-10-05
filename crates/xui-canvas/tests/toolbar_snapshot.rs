//! A LazyRAD-style toolbar rendered headlessly: compact icon buttons packed
//! from the left, group separators, and an empty tail. Pixel assertions check
//! the layout the painter used; the PNGs (`target/snapshots/toolbar-after-*`)
//! are for a human to look at.

use xui_canvas::snapshot::{Snapshot, try_render};
use xui_core::app::{App, Ui};
use xui_core::icon::Lucide;
use xui_core::image::Image;
use xui_core::prelude::{Build, LayoutExt, column, toolbar};
use xui_core::widget::Toolbar;
use xui_core::{Dip, Theme};

struct Host;

impl App for Host {
    type Msg = ();

    fn update(&mut self, _msg: (), _ui: &mut Ui<()>) {}
}

/// New, Open, Save, Save All | Undo, Redo | Cut, Copy, Paste | Run, End.
fn lazyrad_toolbar() -> Build<Toolbar<()>, ()> {
    toolbar()
        .item(Lucide::FilePlus, "New")
        .item(Lucide::FolderOpen, "Open")
        .item(Lucide::Save, "Save")
        .item(Lucide::SaveAll, "Save All")
        .separator()
        .item(Lucide::Undo2, "Undo")
        .item(Lucide::Redo2, "Redo")
        .separator()
        .item(Lucide::Scissors, "Cut")
        .item(Lucide::Copy, "Copy")
        .item(Lucide::ClipboardPaste, "Paste")
        .separator()
        .item_with_text(Lucide::Play, "Run", "Run")
        .item_with_text(Lucide::Square, "End", "End")
}

fn render(theme: Theme, dpi: u32) -> Image {
    try_render(
        Snapshot::new(Dip(720.0), Dip(40.0)).theme(theme).dpi(dpi),
        |ui| {
            ui.root(column().child(lazyrad_toolbar().fill(1)))?;
            Ok(Host)
        },
    )
    .expect("render")
}

/// Whether any pixel with `x` in `xs` and `y` in `ys` differs from `background`.
fn painted(image: &Image, xs: (u32, u32), ys: (u32, u32), background: [u8; 4]) -> bool {
    (ys.0..ys.1).any(|y| (xs.0..xs.1).any(|x| image.pixel(x, y) != Some(background)))
}

#[test]
fn the_toolbar_is_compact_grouped_and_left_aligned() {
    std::fs::create_dir_all("../../target/snapshots").expect("snapshot dir");
    for (name, theme) in [("light", Theme::light()), ("dark", Theme::dark())] {
        let image = render(theme, 96);
        image
            .save_png(format!("../../target/snapshots/toolbar-after-{name}.png"))
            .expect("save");
        let bg = image.pixel(719, 39).expect("background");
        let mid = (18, 22);

        // Icon-only buttons are 40 px squares from x = 0; each icon lands
        // inside its own span. Separators sit at 160 + 4 (and after each group).
        let spans = [
            (0, 40),
            (40, 80),
            (80, 120),
            (120, 160),
            (169, 209),
            (209, 249),
            (258, 298),
            (298, 338),
            (338, 378),
        ];
        for (index, &(start, end)) in spans.iter().enumerate() {
            assert!(
                painted(&image, (start + 10, end - 10), (10, 30), bg),
                "{name}: item {index} paints its icon inside {start}..{end}"
            );
            assert!(
                !painted(&image, (start, start + 8), (0, 40), bg)
                    && !painted(&image, (end - 8, end), (0, 40), bg),
                "{name}: item {index} keeps its icon off the span edges"
            );
        }

        // Each separator line is one pixel wide at its layout x, with the
        // margins on both sides empty.
        for line in [164u32, 253, 382] {
            assert!(
                painted(&image, (line, line + 1), mid, bg),
                "{name}: separator at {line}"
            );
            assert!(
                !painted(&image, (line - 4, line), mid, bg)
                    && !painted(&image, (line + 1, line + 5), mid, bg),
                "{name}: margins around the separator at {line} are empty"
            );
            assert!(
                !painted(&image, (line, line + 1), (0, 8), bg),
                "{name}: the separator stops short of the strip edge"
            );
        }

        // The two labelled buttons follow the last separator, and everything
        // right of them is background.
        assert!(
            painted(&image, (387, 520), (10, 30), bg),
            "{name}: Run, End"
        );
        assert!(
            !painted(&image, (560, 720), (0, 40), bg),
            "{name}: the tail is empty background"
        );
    }
}

#[test]
fn sizes_scale_at_144_dpi() {
    let image = render(Theme::light(), 144);
    let bg = image.pixel(1079, 59).expect("background");
    // The second icon-only button spans 60..120 at 144 DPI.
    assert!(painted(&image, (70, 110), (15, 45), bg));
    assert!(!painted(&image, (60, 68), (0, 60), bg));
    // The first separator: 240 + 6 (margin), two pixels wide.
    assert!(painted(&image, (246, 248), (28, 32), bg));
    assert!(!painted(&image, (240, 246), (28, 32), bg));
    assert!(!painted(&image, (840, 1080), (0, 60), bg), "empty tail");
}
