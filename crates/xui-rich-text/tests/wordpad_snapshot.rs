//! The wordpad example rendered headlessly, light and dark, with a document
//! that shows the formatting row's states. The PNGs land in
//! `target/snapshots/rich-text-wordpad-{light,dark}.png` for a human to look at.
#![allow(dead_code)]

#[path = "../examples/wordpad/app.rs"]
mod app;
#[path = "../examples/wordpad/commands.rs"]
mod commands;
#[path = "../examples/wordpad/files.rs"]
mod files;
#[path = "../examples/wordpad/table.rs"]
mod table;
#[path = "../examples/wordpad/ui.rs"]
mod ui;

use std::sync::Arc;

use xui_canvas::snapshot::{Snapshot, try_render};
use xui_core::image::Image;
use xui_core::{Dip, Theme};
use xui_rich_text::DocPos;
use xui_rich_text::edit::Command;
use xui_rich_text::model::{Align, BlockKind, CharStylePatch, InlineImage, ListKind, Side, Wrap};

use app::Wordpad;

/// A soft diagonal gradient, so a floated image reads at a glance.
fn picture() -> InlineImage {
    let (w, h) = (160u32, 100u32);
    let mut pixels = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            let t = (x + y) as f32 / (w + h) as f32;
            pixels.extend_from_slice(&[
                (60.0 + 150.0 * t) as u8,
                (110.0 + 80.0 * (1.0 - t)) as u8,
                (190.0 - 60.0 * t) as u8,
                255,
            ]);
        }
    }
    InlineImage {
        image: Arc::new(Image::from_rgba(w, h, pixels).expect("pixels")),
        size: (Dip(160.0), Dip(100.0)),
        wrap: Wrap::square(Side::Right),
        alt: "gradient".into(),
    }
}

const BODY: &str = "Wordpad is a small word processor built on xui-rich-text. \
It edits styled text with bold, italic, underlined and struck-through runs, \
paragraph alignment, lists and images that the text flows around, and it \
follows the light or dark theme.";

/// Fills the editor with a document that uses most of the toolbar.
fn fill(app: &Wordpad) {
    let e = &app.editor;
    e.exec(Command::InsertText(format!(
        "Wordpad\n{BODY}\nLists\nBullets keep their level\nNumbers count for you\nNumbers follow on\n\
         A quotation sits apart from the body text.\nLast paragraph, centred."
    )));
    let para = |i: usize| e.exec(Command::SelectParagraph(DocPos::new(i, 0)));
    para(0);
    e.exec(Command::SetBlockKind(BlockKind::Heading(1)));
    para(2);
    e.exec(Command::SetBlockKind(BlockKind::Heading(2)));
    para(3);
    e.exec(Command::ToggleList(ListKind::Bullet));
    para(4);
    e.exec(Command::ToggleList(ListKind::Numbered));
    para(5);
    e.exec(Command::ToggleList(ListKind::Numbered));
    para(6);
    e.exec(Command::SetBlockKind(BlockKind::Quote));
    para(7);
    e.exec(Command::SetAlign(Align::Center));

    let word = |at: usize, command: Command| {
        e.exec(Command::SelectWord(DocPos::new(1, at)));
        e.exec(command);
    };
    word(BODY.find("bold").unwrap(), Command::ToggleBold);
    word(BODY.find("bold").unwrap(), Command::ToggleItalic);
    word(BODY.find("italic").unwrap(), Command::ToggleItalic);
    word(BODY.find("underlined").unwrap(), Command::ToggleUnderline);
    word(BODY.find("struck").unwrap(), Command::ToggleStrike);
    e.exec(Command::SelectWord(DocPos::new(
        1,
        BODY.find("small").unwrap(),
    )));
    e.exec(Command::SetCharStyle(CharStylePatch::bold(true)));

    e.exec(Command::SetCaret {
        pos: DocPos::new(1, 0),
        extend: false,
    });
    e.exec(Command::InsertImage(picture()));
    add_table(app);
    e.set_scroll(0.0);
}

/// A table with a header row before the quotation, the caret in its last
/// cell so the table row is enabled.
fn add_table(app: &Wordpad) {
    let e = &app.editor;
    e.exec(Command::SetCaret {
        pos: DocPos::new(6, 0),
        extend: false,
    });
    e.exec(Command::InsertTable {
        rows: 3,
        columns: 3,
    });
    let cells = [
        "Feature",
        "Shortcut",
        "Notes",
        "Bold",
        "Ctrl+B",
        "Toggles",
        "Next cell",
        "Tab",
        "Adds a row at the end",
    ];
    for (i, text) in cells.iter().enumerate() {
        if i > 0 {
            e.exec(Command::NextCell);
        }
        e.exec(Command::InsertText((*text).into()));
    }
    let cursor = e.table_cursor().expect("in the table");
    let mut table = cursor.table;
    table.header = true;
    e.exec(Command::SetTable {
        id: cursor.id,
        table: table.with_widths(&[1.0, 1.0, 2.0]),
    });
    app.table.sync(e.table_cursor().as_ref());
}

fn render(theme: Theme) -> Image {
    try_render(Snapshot::new(Dip(1100.0), Dip(720.0)).theme(theme), |ui| {
        let app = ui::build(ui)?;
        fill(&app);
        Ok(app)
    })
    .expect("render")
}

#[test]
fn the_wordpad_renders_in_light_and_dark() {
    std::fs::create_dir_all("../../target/snapshots").expect("snapshot dir");
    let light = render(Theme::light());
    let dark = render(Theme::dark());
    for (name, image) in [("light", &light), ("dark", &dark)] {
        image
            .save_png(format!(
                "../../target/snapshots/rich-text-wordpad-{name}.png"
            ))
            .expect("save");
    }
    assert_eq!(light.size(), dark.size());
    assert_ne!(light.pixels(), dark.pixels());
    let corner = |image: &Image| image.pixel(5, 600);
    assert_ne!(
        corner(&light),
        corner(&dark),
        "the document follows the theme"
    );
}
