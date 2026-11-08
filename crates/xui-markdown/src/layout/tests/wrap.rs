#![forbid(unsafe_code)]

//! Code lines wider than their block wrap onto rows. A code character is
//! 6.5 px and the box insets its text by 8 px, so a block 81 px wide holds ten
//! characters a row.

use super::*;
use crate::highlight::{CodeClass, CodeSpan, Highlighter};

/// Ten code characters a row.
const TEN: f32 = 16.0 + 65.0;

/// Marks every `"..."` as a string.
struct Strings;

impl Highlighter for Strings {
    fn line(&self, _lang: &str, line: &str, _state: u32) -> (Vec<CodeSpan>, u32) {
        let mut spans = Vec::new();
        let mut open = None;
        for (at, c) in line.char_indices() {
            if c == '"' {
                match open.take() {
                    Some(start) => spans.push(CodeSpan {
                        range: start..at + 1,
                        class: CodeClass::String,
                    }),
                    None => open = Some(at),
                }
            }
        }
        (spans, 0)
    }
}

#[test]
fn a_long_line_is_cut_into_rows_that_fit() {
    let block = one("```\nabcdefghijklmnopqrstuvwxy\nz\n```", 0, TEN);
    assert_eq!(texts(&block), ["abcdefghij", "klmnopqrst", "uvwxy", "z"]);
    let ys: Vec<f32> = block.pieces.iter().map(|p| p.y).collect();
    assert_eq!(ys, [8.0, 24.0, 40.0, 56.0]);
    assert!(block.pieces.iter().all(|p| p.x == 8.0));
    assert_eq!(block.height, 8.0 + 4.0 * 16.0 + 8.0);
    // Each row maps back to its own bytes of the document.
    let ranges: Vec<_> = block.pieces.iter().map(|p| p.range.clone()).collect();
    let start = ranges[0].start;
    assert_eq!(ranges[1], start + 10..start + 20);
    assert_eq!(ranges[3], start + 26..start + 27);
}

#[test]
fn a_row_ends_after_a_space_in_its_second_half() {
    let block = one("```\naaaaaa bbbbbbbb\n```", 0, TEN);
    assert_eq!(texts(&block), ["aaaaaa ", "bbbbbbbb"]);
    // A space in the first half would leave a short row: the cut stays.
    let block = one("```\naa bbbbbbbbbbbb\n```", 0, TEN);
    assert_eq!(texts(&block), ["aa bbbbbbb", "bbbbb"]);
}

#[test]
fn a_box_narrower_than_a_character_still_shows_one_a_row() {
    let block = one("```\nabc\n```", 0, 18.0);
    assert_eq!(texts(&block), ["a", "b", "c"]);
}

#[test]
fn a_line_that_fits_is_one_piece() {
    let block = one("```\nabcdefghij\n```", 0, TEN);
    assert_eq!(texts(&block), ["abcdefghij"]);
}

#[test]
fn a_span_cut_by_a_row_keeps_its_class_on_both() {
    let mut layout = lay("```\nx = \"a long string\"\n```", TEN);
    layout.set_highlighter(Some(Box::new(Strings)));
    layout.ensure(0.0, 1.0e6);
    let block = layout.block(0).expect("laid out").clone();
    let text = &layout.doc().text;
    let shown: Vec<(&str, Role, f32)> = block
        .pieces
        .iter()
        .map(|p| {
            let range = p.range.start as usize..p.range.end as usize;
            (&text[range], p.role, p.y)
        })
        .collect();
    let string = Role::Code(CodeClass::String);
    assert_eq!(
        shown,
        [
            ("x =", Role::Text, 8.0),
            ("\"a ", string, 8.0),
            // No space in the row's second half: cut at the edge.
            ("long strin", string, 24.0),
            ("g\"", string, 40.0),
        ]
    );
    // Every row starts at the box's left edge.
    assert_eq!(block.pieces[2].x, 8.0);
    assert_eq!(block.pieces[3].x, 8.0);
}

#[test]
fn wrapping_off_clips_the_line_instead() {
    let mut layout = lay("```\nabcdefghijklmnop\n```", TEN);
    layout.set_style(MarkdownStyle {
        wrap_code: false,
        ..MarkdownStyle::default()
    });
    layout.resize(TEN, 96);
    layout.ensure(0.0, 1.0e6);
    let block = layout.block(0).expect("laid out");
    assert_eq!(texts(block), ["abcdefghijklmnop"]);
    assert_eq!(block.clip, Some(FRect::new(8.0, 0.0, 65.0, 32.0)));
}

#[test]
fn the_estimate_counts_wrapped_rows() {
    let short = lay("```\nab\n```", TEN).height();
    let long = lay(&format!("```\n{}\n```", "a".repeat(100)), TEN).height();
    assert!(long > short * 4.0, "{long} vs {short}");
}
