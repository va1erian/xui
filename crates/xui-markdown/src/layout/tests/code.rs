#![forbid(unsafe_code)]

//! Code blocks laid out with and without a highlighter.

use super::*;
use crate::highlight::{CodeClass, CodeSpan, Highlighter};
use crate::layout::code::original_offset;

/// Marks `fn` and `let` as keywords, `"..."` as strings, and `//` and
/// `/* */` as comments; a block comment open at a line's end is state 1.
struct Toy;

impl Highlighter for Toy {
    fn line(&self, _lang: &str, line: &str, state: u32) -> (Vec<CodeSpan>, u32) {
        let mut spans = Vec::new();
        let mut at = 0;
        let mut open = state == 1;
        let push = |spans: &mut Vec<CodeSpan>, range, class| spans.push(CodeSpan { range, class });
        while at < line.len() {
            let rest = &line[at..];
            if open || rest.starts_with("/*") {
                let body = if open { 0 } else { 2 };
                let end = rest[body..].find("*/").map(|i| at + body + i + 2);
                open = end.is_none();
                let end = end.unwrap_or(line.len());
                push(&mut spans, at..end, CodeClass::Comment);
                at = end;
            } else if rest.starts_with("//") {
                push(&mut spans, at..line.len(), CodeClass::Comment);
                break;
            } else if let Some(body) = rest.strip_prefix('"') {
                let end = body.find('"').map_or(line.len(), |i| at + i + 2);
                push(&mut spans, at..end, CodeClass::String);
                at = end;
            } else if rest.starts_with("fn") || rest.starts_with("let") {
                let len = if rest.starts_with("fn") { 2 } else { 3 };
                let word = !rest[len..].starts_with(|c: char| c.is_alphanumeric());
                if word && !line[..at].ends_with(|c: char| c.is_alphanumeric()) {
                    push(&mut spans, at..at + len, CodeClass::Keyword);
                }
                at += len;
            } else {
                at += rest.chars().next().map_or(1, char::len_utf8);
            }
        }
        (spans, u32::from(open))
    }
}

/// The same spans for every line.
struct Fixed(Vec<CodeSpan>);

impl Highlighter for Fixed {
    fn line(&self, _: &str, _: &str, _: u32) -> (Vec<CodeSpan>, u32) {
        (self.0.clone(), 0)
    }
}

fn span(range: Range<usize>, class: CodeClass) -> CodeSpan {
    CodeSpan { range, class }
}

fn with(markdown: &str, highlighter: impl Highlighter + 'static) -> (Layout, BlockLayout) {
    let mut layout = lay(markdown, 1000.0);
    layout.set_highlighter(Some(Box::new(highlighter)));
    layout.ensure(0.0, 1.0e6);
    let block = layout.block(0).expect("laid out").clone();
    (layout, block)
}

/// What each piece shows, by its range, with its role.
fn shown(layout: &Layout, block: &BlockLayout) -> Vec<(String, Role)> {
    let text = &layout.doc().text;
    block
        .pieces
        .iter()
        .map(|p| {
            let range = p.range.start as usize..p.range.end as usize;
            (text[range].to_owned(), p.role)
        })
        .collect()
}

fn code(class: CodeClass) -> Role {
    Role::Code(class)
}

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() <= 0.5
}

#[test]
fn without_a_highlighter_a_line_is_one_piece() {
    let block = one("```rust\nfn a() {}\n\nlet b\n```", 0, 1000.0);
    assert_eq!(texts(&block), ["fn a() {}", "let b"]);
    assert!(block.pieces.iter().all(|p| p.role == Role::Text));
}

#[test]
fn a_highlighted_line_is_classed_pieces_with_plain_text_between() {
    let (layout, block) = with("```\nfn main() // go\n```", Toy);
    assert_eq!(
        shown(&layout, &block),
        [
            ("fn".to_owned(), code(CodeClass::Keyword)),
            ("main()".to_owned(), Role::Text),
            ("// go".to_owned(), code(CodeClass::Comment)),
        ]
    );
    assert_eq!(texts(&block), ["fn", "main()", "// go"]);
}

#[test]
fn pieces_sit_where_the_whole_line_puts_them() {
    let (_, block) = with("```\nlet x = \"a b\" // c\n```", Toy);
    // A code character is 6.5 px and the box insets the text by 8 px.
    let starts = [0.0, 4.0, 8.0, 14.0];
    assert_eq!(block.pieces.len(), 4, "{:?}", texts(&block));
    for (p, col) in block.pieces.iter().zip(starts) {
        assert!(near(p.x, 8.0 + col * 6.5), "{} at {}", text(p), p.x);
        assert_eq!(p.y, 8.0);
    }
    assert!(block.pieces.windows(2).all(|w| w[0].x < w[1].x));
}

#[test]
fn trailing_space_inside_a_span_keeps_the_next_piece_in_place() {
    let (_, block) = with(
        "```\nab   cd\n```",
        Fixed(vec![span(0..5, CodeClass::Type)]),
    );
    assert_eq!(texts(&block), ["ab   ", "cd"]);
    assert!(near(block.pieces[1].x, 8.0 + 5.0 * 6.5));
}

#[test]
fn plain_stretches_are_trimmed_and_positioned_at_their_trimmed_start() {
    let (layout, block) = with("```\n  a  fn  b  \n```", Toy);
    assert_eq!(
        shown(&layout, &block),
        [
            ("a".to_owned(), Role::Text),
            ("fn".to_owned(), code(CodeClass::Keyword)),
            ("b".to_owned(), Role::Text),
        ]
    );
    assert!(near(block.pieces[0].x, 8.0 + 2.0 * 6.5));
    assert!(near(block.pieces[2].x, 8.0 + 9.0 * 6.5));
}

#[test]
fn the_state_is_carried_from_line_to_line() {
    let (layout, block) = with("```\n/* open\nfn inside\nclose */ fn\nfn\n```", Toy);
    let classes: Vec<_> = shown(&layout, &block).into_iter().map(|(_, r)| r).collect();
    assert_eq!(
        classes,
        [
            code(CodeClass::Comment),
            code(CodeClass::Comment),
            code(CodeClass::Comment),
            code(CodeClass::Keyword),
            code(CodeClass::Keyword),
        ]
    );
    let ys: Vec<f32> = block.pieces.iter().map(|p| p.y).collect();
    assert_eq!(ys, [8.0, 24.0, 40.0, 40.0, 56.0]);
}

#[test]
fn the_language_reaches_the_highlighter() {
    struct Lang;
    impl Highlighter for Lang {
        fn line(&self, lang: &str, line: &str, _: u32) -> (Vec<CodeSpan>, u32) {
            let class = if lang == "rust" {
                CodeClass::Type
            } else {
                CodeClass::Number
            };
            (vec![span(0..line.len(), class)], 0)
        }
    }
    let (_, rust) = with("```rust\nx\n```", Lang);
    assert_eq!(rust.pieces[0].role, code(CodeClass::Type));
    let (_, none) = with("```\nx\n```", Lang);
    assert_eq!(none.pieces[0].role, code(CodeClass::Number));
}

#[test]
#[allow(clippy::reversed_empty_ranges)]
fn bad_spans_are_skipped() {
    let spans = vec![
        span(2..4, CodeClass::Keyword),
        span(1..3, CodeClass::String),
        span(5..5, CodeClass::Number),
        span(6..2, CodeClass::Number),
        span(4..5, CodeClass::Type),
        span(0..1, CodeClass::Comment),
        span(8..99, CodeClass::Type),
    ];
    let (layout, block) = with("```\nabcdefgh\n```", Fixed(spans));
    assert_eq!(
        shown(&layout, &block),
        [
            ("ab".to_owned(), Role::Text),
            ("cd".to_owned(), code(CodeClass::Keyword)),
            ("e".to_owned(), code(CodeClass::Type)),
            ("fgh".to_owned(), Role::Text),
        ]
    );
}

#[test]
fn spans_off_char_boundaries_are_skipped() {
    let spans = vec![
        span(1..2, CodeClass::Keyword),
        span(0..1, CodeClass::String),
    ];
    let (layout, block) = with("```\n\u{e9}a\n```", Fixed(spans));
    assert_eq!(shown(&layout, &block), [("\u{e9}a".to_owned(), Role::Text)]);
}

#[test]
fn ranges_of_a_line_with_tabs_are_the_original_bytes() {
    let (layout, block) = with("```\nx\n\tfn\t\"a\tb\" // c\n```", Toy);
    assert_eq!(
        shown(&layout, &block),
        [
            ("x".to_owned(), Role::Text),
            ("fn".to_owned(), code(CodeClass::Keyword)),
            ("\"a\tb\"".to_owned(), code(CodeClass::String)),
            ("// c".to_owned(), code(CodeClass::Comment)),
        ]
    );
    assert_eq!(
        texts(&block)[2],
        "\"a    b\"",
        "shaped with the tab widened"
    );
    // After one tab (four columns) comes `fn`, then another tab, then the
    // string.
    assert!(near(block.pieces[1].x, 8.0 + 4.0 * 6.5));
    assert!(near(block.pieces[2].x, 8.0 + 10.0 * 6.5));
}

#[test]
fn a_hit_in_a_tab_expanded_piece_maps_back_through_the_range() {
    let (layout, block) = with("```\n\tfn\t\"a\tb\"\n```", Toy);
    let string = block
        .pieces
        .iter()
        .find(|p| p.role == code(CodeClass::String))
        .expect("a string piece");
    let source = &layout.doc().text[string.range.start as usize..string.range.end as usize];
    assert_eq!(source, "\"a\tb\"");
    // The tab at original byte 2 is shaped as bytes 2..6.
    assert_eq!(original_offset(source, 2), 2, "before the tab");
    assert_eq!(original_offset(source, 3), 2, "nearer the front");
    assert_eq!(original_offset(source, 4), 3, "nearer the back");
    assert_eq!(original_offset(source, 6), 3, "after the tab");
    assert_eq!(original_offset(source, 100), source.len());
    // Through the shaper's own hit test: column 6 is just before the `b`.
    let hit = string.layout.hit_test_point(6.5 * 6.0, 0.0);
    let at = string.range.start as usize + original_offset(source, hit.byte_index);
    assert_eq!(&layout.doc().text[at..at + 2], "b\"");
}

#[test]
fn quoted_code_keeps_its_plain_text_weak_and_its_spans_coloured() {
    let (layout, block) = with("> ```\n> fn x\n> ```", Toy);
    assert_eq!(
        shown(&layout, &block),
        [
            ("fn".to_owned(), code(CodeClass::Keyword)),
            ("x".to_owned(), Role::Weak),
        ]
    );
    let plain = one("> ```\n> fn x\n> ```", 0, 1000.0);
    assert_eq!(plain.pieces[0].role, Role::Weak);
}

#[test]
fn empty_and_blank_lines_make_no_pieces_but_take_their_height() {
    let (layout, block) = with("```\nfn\n\n   \nlet\n```", Toy);
    assert_eq!(
        shown(&layout, &block),
        [
            ("fn".to_owned(), code(CodeClass::Keyword)),
            ("let".to_owned(), code(CodeClass::Keyword)),
        ]
    );
    assert_eq!(block.pieces[1].y, 8.0 + 3.0 * 16.0);
    assert_eq!(block.height, 8.0 + 4.0 * 16.0 + 8.0);
    let plain = one("```\nfn\n\n   \nlet\n```", 0, 1000.0);
    assert_eq!(plain.height, block.height);
}

#[test]
fn a_highlighted_block_keeps_its_box_and_clip() {
    let (_, block) = with("```\nfn\n```", Toy);
    assert_eq!(
        block.decor[0],
        Decor::Fill(FRect::new(0.0, 0.0, 1000.0, 32.0), Role::Surface)
    );
    assert_eq!(block.clip, Some(FRect::new(8.0, 0.0, 984.0, 32.0)));
}

#[test]
fn original_offsets_skip_widened_tabs() {
    let text = "a\tb\u{e9}\t";
    assert_eq!(original_offset(text, 0), 0);
    assert_eq!(original_offset(text, 1), 1);
    assert_eq!(original_offset(text, 5), 2);
    assert_eq!(original_offset(text, 6), 3);
    assert_eq!(original_offset(text, 8), 5);
    assert_eq!(original_offset(text, 100), text.len());
}

#[test]
fn the_state_is_carried_across_a_blank_line() {
    let (layout, block) = with("```\n/* open\n\nclose */ fn\n```", Toy);
    let classes: Vec<_> = shown(&layout, &block).into_iter().map(|(_, r)| r).collect();
    assert_eq!(
        classes,
        [
            code(CodeClass::Comment),
            code(CodeClass::Comment),
            code(CodeClass::Keyword),
        ]
    );
}
