#![forbid(unsafe_code)]

//! Line start and end, vertical movement and paging.

use xui_core::{Color, Dip};

use super::*;
use crate::model::{DocPos, InlineImage, ParaStyle, ParaStyleId, Side, Wrap};

const WRAPPED: &str = "aaaa bbbb cccc dddd eeee";

#[test]
fn home_and_end_stay_on_the_visual_line() {
    let doc = plain(&[WRAPPED]);
    let layout = lay(&doc, 140.0);
    let at = |byte| DocPos::new(0, byte);
    assert_eq!(layout.line_start(at(5)), at(0));
    assert_eq!(layout.line_end(at(5)), at(19), "before the hanging space");
    assert_eq!(layout.line_start(at(22)), at(20));
    assert_eq!(layout.line_end(at(22)), at(24));
    // A position at the wrap belongs to the later line, as the caret draws it.
    assert_eq!(layout.line_start(at(20)), at(20));
}

#[test]
fn end_stops_before_a_forced_break() {
    let doc = plain(&["ab\u{2028}cd"]);
    let layout = lay(&doc, 140.0);
    assert_eq!(layout.line_end(DocPos::new(0, 0)), DocPos::new(0, 2));
    assert_eq!(layout.line_start(DocPos::new(0, 5)), DocPos::new(0, 5));
    assert_eq!(layout.line_end(DocPos::new(0, 5)), DocPos::new(0, 7));
}

#[test]
fn an_empty_paragraph_is_its_own_line() {
    let doc = plain(&["abc", "", "def"]);
    let layout = lay(&doc, 140.0);
    let empty = DocPos::new(1, 0);
    assert_eq!(layout.line_start(empty), empty);
    assert_eq!(layout.line_end(empty), empty);
}

#[test]
fn moving_down_keeps_the_column_through_a_short_line() {
    let doc = plain(&["aaaaaaaaaa", "ab", "aaaaaaaaaa"]);
    let layout = lay(&doc, 140.0);
    let (pos, x) = layout.vertical(DocPos::new(0, 8), None, 1);
    assert_eq!((pos, x), (DocPos::new(1, 2), 56.0));
    let (pos, x) = layout.vertical(pos, Some(x), 1);
    assert_eq!((pos, x), (DocPos::new(2, 8), 56.0));
    let (pos, _) = layout.vertical(pos, Some(x), -2);
    assert_eq!(pos, DocPos::new(0, 8));
}

#[test]
fn moving_within_a_wrapped_paragraph_and_at_the_ends() {
    let doc = plain(&[WRAPPED]);
    let layout = lay(&doc, 140.0);
    let (down, x) = layout.vertical(DocPos::new(0, 2), None, 1);
    assert_eq!((down, x), (DocPos::new(0, 22), 14.0));
    let (stuck, _) = layout.vertical(down, Some(x), 1);
    assert_eq!(stuck, down, "the last line does not move");
    let (stuck, _) = layout.vertical(DocPos::new(0, 2), None, -1);
    assert_eq!(stuck, DocPos::new(0, 2), "the first line does not move");
    let (far, _) = layout.vertical(DocPos::new(0, 2), None, 50);
    assert_eq!(far, down, "a long move stops at the last line");
}

#[test]
fn a_column_left_of_the_text_beside_a_float_lands_at_the_line_start() {
    let mut b = DocBuilder::new();
    let img = b.object(InlineImage {
        image: gradient_image(8, 8, Color::rgb(0, 0, 0), Color::rgb(9, 9, 9)),
        size: (Dip(60.0), Dip(50.0)),
        wrap: Wrap::square(Side::Left),
        alt: String::new(),
    });
    let text = "aaaa bbbb cccc dddd eeee ffff gggg hhhh iiii jjjj kkkk llll mmmm nnnn oooo \
                pppp qqqq rrrr ssss tttt uuuu vvvv wwww xxxx yyyy zzzz 1111 2222 3333 4444 5555 6666";
    b.paragraph(
        ParaStyleId::DEFAULT,
        &[Run::Object(img), Run::Text(text, CharStyleId::DEFAULT)],
    );
    let doc = b.finish();
    let layout = lay(&doc, 300.0);
    let lines = &layout.paragraphs()[0].lines;
    let below = lines.iter().position(|l| l.y >= 58.0).unwrap();
    assert!(below >= 2);
    // From x = 14, left of the float's right edge (68), up into the narrow
    // lines: the nearest text is the start of each line.
    let from = DocPos::new(0, lines[below].range.start + 2);
    let (up, x) = layout.vertical(from, None, -1);
    assert_eq!(x, 14.0);
    assert_eq!(
        up,
        layout.line_start(up),
        "the start of the line beside the float"
    );
    assert_eq!(up.byte, lines[below - 1].range.start);
    // And from beside the float down to full width again, x is kept.
    let beside = DocPos::new(0, lines[below - 1].range.start + 6);
    let (down, x) = layout.vertical(beside, None, 1);
    assert_eq!(x, 68.0 + 42.0, "the caret's own x beside the float");
    assert_eq!(down.byte, lines[below].range.start + 16);
}

#[test]
fn different_indents_in_neighbouring_paragraphs_keep_the_column() {
    let mut b = DocBuilder::new();
    let indented = b.para_style(ParaStyle {
        indent_left: Dip(28.0),
        ..ParaStyle::default()
    });
    b.paragraph(indented, &[Run::Text("indented", CharStyleId::DEFAULT)]);
    b.paragraph(
        ParaStyleId::DEFAULT,
        &[Run::Text("flush text", CharStyleId::DEFAULT)],
    );
    let doc = b.finish();
    let layout = lay(&doc, 300.0);
    // Column 21 is left of the indented text: its start.
    let (up, x) = layout.vertical(DocPos::new(1, 3), None, -1);
    assert_eq!((up, x), (DocPos::new(0, 0), 21.0));
    // Column 49 is two characters into the indented line.
    let (up, x) = layout.vertical(DocPos::new(1, 7), None, -1);
    assert_eq!((up, x), (DocPos::new(0, 3), 49.0));
    let (down, _) = layout.vertical(up, Some(x), 1);
    assert_eq!(down, DocPos::new(1, 7));
}

#[test]
fn a_page_is_the_lines_that_fit_less_one() {
    let doc = plain(&["a", "b", "c"]);
    let layout = lay(&doc, 140.0);
    assert_eq!(layout.page_lines(180.0), 9);
    assert_eq!(layout.page_lines(10.0), 1);
    assert_eq!(Layout::new().page_lines(500.0), 1);
}
