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
        // The inline image fills (8, 8) to (68, 48).
        mouse(stage, "down", 30, 20);
        mouse(stage, "up", 30, 20);
        assert!(matches!(rig.editor.selection(), Selection::Object(_)));

        // South-east corner handle, dragged by (30, 20): the aspect is kept.
        mouse(stage, "down", 68, 48);
        mouse(stage, "move", 80, 58);
        mouse(stage, "move", 98, 68);
        assert_eq!(image_size(rig), (90.0, 60.0), "previewed while dragging");
        mouse(stage, "up", 98, 68);
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
        mouse(stage, "down", 68, 48);
        mouse(stage, "move", 98, 68);
        assert_eq!(image_size(rig), (90.0, 60.0));
        key(stage, Key::ESCAPE);
        assert_eq!(image_size(rig), (60.0, 40.0));
        mouse(stage, "up", 98, 68);
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

#[test]
fn the_handles_of_an_image_in_the_corner_stay_inside_the_view() {
    let image = run_themed(Theme::light(), image_doc(), |stage, _| {
        mouse(stage, "down", 30, 30);
        mouse(stage, "up", 30, 30);
    });
    // Handles are 8 px squares centred on the image's corners at (8, 8).
    // The stroke is antialiased, so look for blue-tinted pixels near the edge.
    let blue = |x: u32, y: u32| {
        image
            .pixel(x, y)
            .is_some_and(|p| p[2] > p[0].saturating_add(40))
    };
    let top_row = (4..12).filter(|&x| (3..=5).any(|y| blue(x, y))).count();
    let left_column = (4..12).filter(|&y| (3..=5).any(|x| blue(x, y))).count();
    assert!(top_row >= 6, "the top-left handle's top edge: {top_row}");
    assert!(left_column >= 6, "and its left edge: {left_column}");
}

#[test]
fn dragging_after_a_double_click_extends_by_whole_words() {
    run(plain("alpha beta gamma delta"), |stage, rig| {
        let at = |byte| {
            let r = rig
                .editor
                .caret_rect(DocPos::new(0, byte), Default::default());
            (r.left + 1, (r.top + r.bottom) / 2)
        };
        let (x, y) = at(8);
        mouse(stage, "down", x, y);
        mouse(stage, "up", x, y);
        mouse(stage, "double", x, y);
        let (x, y) = at(18);
        mouse(stage, "move", x, y);
        let range = rig.range();
        assert_eq!(
            (range.start.byte, range.end.byte),
            (6, 22),
            "to the end of delta"
        );
        let (x, y) = at(2);
        mouse(stage, "move", x, y);
        let range = rig.range();
        assert_eq!(
            (range.start.byte, range.end.byte),
            (0, 10),
            "beta stays selected"
        );
        mouse(stage, "up", x, y);
    });
}

#[test]
fn dragging_after_a_triple_click_extends_by_whole_paragraphs() {
    let mut b = DocBuilder::new();
    for text in ["one two", "three four", "five six"] {
        b.paragraph(
            ParaStyleId::DEFAULT,
            &[Run::Text(text, CharStyleId::DEFAULT)],
        );
    }
    run(b.finish(), |stage, rig| {
        let at = |para, byte| {
            let r = rig
                .editor
                .caret_rect(DocPos::new(para, byte), Default::default());
            (r.left + 1, (r.top + r.bottom) / 2)
        };
        let (x, y) = at(1, 2);
        mouse(stage, "down", x, y);
        mouse(stage, "up", x, y);
        mouse(stage, "double", x, y);
        mouse(stage, "up", x, y);
        mouse(stage, "down", x, y);
        let (x, y) = at(2, 2);
        mouse(stage, "move", x, y);
        let range = rig.range();
        assert_eq!(
            (range.start, range.end),
            (DocPos::new(1, 0), DocPos::new(2, 8))
        );
        let (x, y) = at(0, 1);
        mouse(stage, "move", x, y);
        let range = rig.range();
        assert_eq!(
            (range.start, range.end),
            (DocPos::new(0, 0), DocPos::new(1, 10))
        );
        mouse(stage, "up", x, y);
    });
}
