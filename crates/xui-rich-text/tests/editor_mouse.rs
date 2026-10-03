//! Mouse input: caret, selection, images, and the themed overlays.

mod common;
mod rig;

use common::{DocBuilder, Run};
use rig::*;
use xui_core::Theme;
use xui_core::message::Key;
use xui_rich_text::DocPos;
use xui_rich_text::model::{CharStyleId, ParaStyleId, Selection};

#[test]
fn a_click_places_the_caret() {
    run(plain("hello world"), |stage, rig| {
        let at = rig.editor.caret_rect(DocPos::new(0, 3), Default::default());
        mouse(stage, "down", at.left, (at.top + at.bottom) / 2);
        mouse(stage, "up", at.left, (at.top + at.bottom) / 2);
        assert_eq!(rig.editor.selection(), Selection::caret(DocPos::new(0, 3)));
    });
}

#[test]
fn a_drag_selects_text() {
    run(plain("hello world"), |stage, rig| {
        let a = rig.editor.caret_rect(DocPos::new(0, 2), Default::default());
        let b = rig.editor.caret_rect(DocPos::new(0, 7), Default::default());
        let y = (a.top + a.bottom) / 2;
        mouse(stage, "down", a.left, y);
        mouse(stage, "move", b.left, y);
        mouse(stage, "up", b.left, y);
        let range = rig.range();
        assert_eq!((range.start.byte, range.end.byte), (2, 7));
    });
}

#[test]
fn double_click_selects_a_word_and_a_third_click_the_paragraph() {
    run(plain("alpha beta gamma"), |stage, rig| {
        let r = rig.editor.caret_rect(DocPos::new(0, 8), Default::default());
        let y = (r.top + r.bottom) / 2;
        mouse(stage, "down", r.left, y);
        mouse(stage, "up", r.left, y);
        mouse(stage, "double", r.left, y);
        mouse(stage, "up", r.left, y);
        let range = rig.range();
        assert_eq!((range.start.byte, range.end.byte), (6, 10));
        mouse(stage, "down", r.left, y);
        mouse(stage, "up", r.left, y);
        let range = rig.range();
        assert_eq!((range.start.byte, range.end.byte), (0, 16));
    });
}
#[test]
fn clicking_an_image_selects_it_and_a_handle_drag_resizes_it_in_one_step() {
    run(image_doc(), |stage, rig| {
        // The inline image fills (8, 0) to (68, 40).
        mouse(stage, "down", 30, 20);
        mouse(stage, "up", 30, 20);
        assert!(matches!(rig.editor.selection(), Selection::Object(_)));

        // South-east corner handle, dragged by (30, 20): the aspect is kept.
        mouse(stage, "down", 68, 40);
        mouse(stage, "move", 80, 50);
        mouse(stage, "move", 98, 60);
        assert_eq!(image_size(rig), (90.0, 60.0), "previewed while dragging");
        mouse(stage, "up", 98, 60);
        assert_eq!(image_size(rig), (90.0, 60.0));
        ctrl(stage, Key::Z);
        assert_eq!(image_size(rig), (60.0, 40.0), "one undo step");
    });
}

#[test]
fn escape_cancels_a_resize() {
    run(image_doc(), |stage, rig| {
        mouse(stage, "down", 30, 20);
        mouse(stage, "up", 30, 20);
        mouse(stage, "down", 68, 40);
        mouse(stage, "move", 98, 60);
        assert_eq!(image_size(rig), (90.0, 60.0));
        key(stage, Key::ESCAPE);
        assert_eq!(image_size(rig), (60.0, 40.0));
        mouse(stage, "up", 98, 60);
        assert_eq!(image_size(rig), (60.0, 40.0));
    });
}

#[test]
fn dragging_an_image_moves_its_anchor() {
    let mut b = DocBuilder::new();
    let img = picture(&mut b, 20.0, 20.0);
    b.paragraph(
        ParaStyleId::DEFAULT,
        &[
            Run::Text("ab", CharStyleId::DEFAULT),
            Run::Object(img),
            Run::Text("cd", CharStyleId::DEFAULT),
        ],
    );
    let anchor = |rig: &Rig| {
        rig.editor
            .with_document(|d| d.paragraphs()[0].objects().next().expect("object").0)
    };
    run(b.finish(), move |stage, rig| {
        assert_eq!(anchor(rig), 2);
        let r = rig.editor.caret_rect(DocPos::new(0, 2), Default::default());
        let (x, y) = (r.left + 8, (r.top + r.bottom) / 2);
        mouse(stage, "down", x, y);
        mouse(stage, "move", 200, y);
        mouse(stage, "move", 380, y);
        mouse(stage, "up", 380, y);
        assert_eq!(anchor(rig), 4, "dropped after the last character");
        ctrl(stage, Key::Z);
        assert_eq!(anchor(rig), 2);
    });
}

#[test]
fn the_caret_selection_and_handles_follow_the_theme() {
    for (name, theme) in [("light", Theme::light()), ("dark", Theme::dark())] {
        let caret = run_themed(theme, plain("hello world"), |stage, _| {
            key(stage, Key::END);
        });
        save(&format!("rich-text-caret-{name}.png"), &caret);
        // The caret is a one-pixel column in the text colour, past the text.
        assert!(has_column(&caret, theme.text, 12), "a caret-tall column");

        let selected = run_themed(theme, plain("hello world"), |stage, _| {
            ctrl(stage, Key::A);
        });
        save(&format!("rich-text-selected-{name}.png"), &selected);
        assert!(pixels_of(&selected, theme.selection) > 300);

        let image = run_themed(theme, image_doc(), |stage, _| {
            mouse(stage, "down", 30, 20);
            mouse(stage, "up", 30, 20);
        });
        save(&format!("rich-text-image-{name}.png"), &image);
        assert!(pixels_of(&image, theme.accent) > 150, "outline and handles");
    }
}
