#![forbid(unsafe_code)]

//! Line breaking, alignment, spacing, baselines and block defaults.

use xui_core::{Color, Dip};

use super::*;
use crate::layout::PlacedKind;
use crate::model::{
    Align, Baseline, BlockKind, CharStyle, InlineImage, LineSpacing, ListItem, ListKind, ParaStyle,
    Wrap,
};

const EPS: f32 = 0.01;

fn aligned(align: Align, text: &'static str) -> Document {
    with_style(
        ParaStyle {
            align,
            ..ParaStyle::default()
        },
        |_| vec![Run::Text(text, CharStyleId::DEFAULT)],
    )
}

fn text_items(layout: &Layout) -> Vec<(f32, f32)> {
    layout.paragraphs()[0].lines[0]
        .items
        .iter()
        .filter(|i| matches!(i.kind, PlacedKind::Text(_)))
        .map(|i| (i.x, i.width))
        .collect()
}

#[test]
fn lines_break_after_the_last_word_that_fits() {
    let doc = plain(&["aaaa bbbb cccc dddd eeee"]);
    let layout = lay(&doc, 140.0);
    assert_eq!(
        line_texts(&doc, &layout, 0),
        ["aaaa bbbb cccc dddd", "eeee"]
    );
}

#[test]
fn a_word_that_exactly_fills_the_line_stays_on_it() {
    let doc = plain(&["aaaa bbbb cccc ddddd"]);
    let layout = lay(&doc, 140.0);
    assert_eq!(line_texts(&doc, &layout, 0), ["aaaa bbbb cccc ddddd"]);
}

#[test]
fn a_word_wider_than_the_line_breaks_at_graphemes() {
    let doc = plain(&["abcdefghijklmnopqrstuvwxyz"]);
    let layout = lay(&doc, 70.0);
    assert_eq!(
        line_texts(&doc, &layout, 0),
        ["abcdefghij", "klmnopqrst", "uvwxyz"]
    );
}

#[test]
fn lines_tile_the_paragraph_text() {
    let doc = plain(&["one two  three four five six seven eight nine ten"]);
    let layout = lay(&doc, 112.0);
    let lines = &layout.paragraphs()[0].lines;
    assert!(lines.len() > 2);
    assert_eq!(lines[0].range.start, 0);
    for pair in lines.windows(2) {
        assert_eq!(pair[0].range.end, pair[1].range.start);
    }
    assert_eq!(
        lines.last().unwrap().range.end,
        doc.paragraphs()[0].text().len()
    );
}

#[test]
fn alignment_places_a_short_line() {
    let at = |align| {
        let doc = aligned(align, "hello");
        text_items(&lay(&doc, 140.0))[0].0
    };
    assert!((at(Align::Left) - 0.0).abs() < EPS);
    assert!((at(Align::Center) - 52.5).abs() < EPS);
    assert!((at(Align::Right) - 105.0).abs() < EPS);
}

#[test]
fn trailing_spaces_do_not_shift_a_right_aligned_line() {
    let doc = aligned(Align::Right, "hello   ");
    assert!((text_items(&lay(&doc, 140.0))[0].0 - 105.0).abs() < EPS);
}

#[test]
fn justify_stretches_every_line_but_the_last() {
    let doc = aligned(Align::Justify, "aaaa bbbb cccc dddd eeee");
    let layout = lay(&doc, 140.0);
    let lines = &layout.paragraphs()[0].lines;
    let words = |line: usize| -> Vec<(f32, f32)> {
        lines[line]
            .items
            .iter()
            .filter(|i| matches!(i.kind, PlacedKind::Text(_)))
            .map(|i| (i.x, i.width))
            .collect()
    };
    let first = words(0);
    let (x, w) = *first.last().unwrap();
    assert!((x + w - 140.0).abs() < EPS, "ends at the edge: {}", x + w);
    assert!(first[1].0 > 5.0 * 7.0, "the gaps grew");
    assert_eq!(words(1), [(0.0, 28.0)], "the last line is not stretched");
}

#[test]
fn a_forced_break_ends_a_line_and_is_not_justified() {
    let doc = aligned(Align::Justify, "aa bb\u{2028}cc dd");
    let layout = lay(&doc, 140.0);
    assert_eq!(layout.paragraphs()[0].lines.len(), 2);
    let first = &layout.paragraphs()[0].lines[0];
    assert!(
        first
            .items
            .iter()
            .map(|i| i.x + i.width)
            .fold(0.0, f32::max)
            < 140.0
    );
}

#[test]
fn a_trailing_forced_break_leaves_an_empty_line() {
    let doc = plain(&["a\u{2028}"]);
    let layout = lay(&doc, 140.0);
    assert_eq!(layout.paragraphs()[0].lines.len(), 2);
    assert!(layout.paragraphs()[0].lines[1].range.is_empty());
}

#[test]
fn an_empty_paragraph_has_one_line_of_its_style() {
    let layout = lay(&plain(&[""]), 140.0);
    let p = &layout.paragraphs()[0];
    assert_eq!(p.lines.len(), 1);
    assert!((p.height - 18.0).abs() < EPS);
}

#[test]
fn mixed_sizes_share_a_baseline() {
    let doc = with_style(ParaStyle::default(), |b| {
        let big = b.char_style(CharStyle {
            size: Dip(28.0),
            ..CharStyle::default()
        });
        vec![
            Run::Text("ab ", CharStyleId::DEFAULT),
            Run::Text("CD", big),
            Run::Text(" ef", CharStyleId::DEFAULT),
        ]
    });
    let layout = lay(&doc, 400.0);
    let line = &layout.paragraphs()[0].lines[0];
    // 28 dip: height 35, baseline 26.25, descent 8.75.
    assert!((line.height - 35.0).abs() < EPS);
    assert!((line.baseline - 26.25).abs() < EPS);
    for item in &line.items {
        if let PlacedKind::Text(t) = &item.kind {
            let top = line.baseline - t.baseline();
            assert!(top >= -EPS && top + t.height() <= line.height + EPS);
            assert!((top + t.baseline() - line.baseline).abs() < EPS);
        }
    }
}

#[test]
fn superscripts_are_smaller_and_raised() {
    let doc = with_style(ParaStyle::default(), |b| {
        let sup = b.char_style(CharStyle {
            baseline: Baseline::Superscript,
            ..CharStyle::default()
        });
        vec![Run::Text("x", CharStyleId::DEFAULT), Run::Text("22", sup)]
    });
    let layout = lay(&doc, 400.0);
    let item = &layout.paragraphs()[0].lines[0].items[1];
    assert!(item.dy < 0.0);
    assert!((item.width - 2.0 * 7.0 * 0.65).abs() < EPS);
}

#[test]
fn an_inline_image_raises_its_line_and_sits_on_the_baseline() {
    let doc = with_style(ParaStyle::default(), |b| {
        let id = b.object(InlineImage {
            image: crate::layout::sample::gradient_image(
                20,
                40,
                Color::rgb(0, 0, 0),
                Color::rgb(9, 9, 9),
            ),
            size: (Dip(20.0), Dip(40.0)),
            wrap: Wrap::Inline,
            alt: String::new(),
        });
        vec![
            Run::Text("ab", CharStyleId::DEFAULT),
            Run::Object(id),
            Run::Text("cd", CharStyleId::DEFAULT),
        ]
    });
    let layout = lay(&doc, 400.0);
    let line = &layout.paragraphs()[0].lines[0];
    assert!(
        (line.baseline - 40.0).abs() < EPS,
        "ascent is the image height"
    );
    assert!((line.height - 44.5).abs() < EPS, "plus the text descent");
    let object = line
        .items
        .iter()
        .find(|i| matches!(i.kind, PlacedKind::Object { .. }))
        .unwrap();
    assert!((object.width - 20.0).abs() < EPS);
}

#[test]
fn line_spacing_scales_or_fixes_the_line_height() {
    let height = |spacing| {
        let doc = with_style(
            ParaStyle {
                line_spacing: spacing,
                ..ParaStyle::default()
            },
            |_| vec![Run::Text("hi", CharStyleId::DEFAULT)],
        );
        lay(&doc, 400.0).paragraphs()[0].lines[0].height
    };
    assert!((height(LineSpacing::Multiple(2.0)) - 36.0).abs() < EPS);
    assert!((height(LineSpacing::Exactly(Dip(30.0))) - 30.0).abs() < EPS);
}

#[test]
fn space_before_and_after_pad_the_paragraph() {
    let doc = with_style(
        ParaStyle {
            space_before: Dip(10.0),
            space_after: Dip(6.0),
            ..ParaStyle::default()
        },
        |_| vec![Run::Text("hi", CharStyleId::DEFAULT)],
    );
    let p = lay(&doc, 400.0).paragraphs()[0].clone();
    assert!((p.lines[0].y - 10.0).abs() < EPS);
    assert!((p.height - 34.0).abs() < EPS);
}

#[test]
fn indents_narrow_the_lines_and_the_first_line_has_its_own() {
    let style = ParaStyle {
        indent_left: Dip(14.0),
        indent_right: Dip(14.0),
        indent_first: Dip(21.0),
        ..ParaStyle::default()
    };
    let doc = with_style(style, |_| {
        vec![Run::Text(
            "aaaa bbbb cccc dddd eeee ffff gggg hhhh",
            CharStyleId::DEFAULT,
        )]
    });
    let layout = lay(&doc, 140.0);
    let lines = &layout.paragraphs()[0].lines;
    assert!((lines[0].items[0].x - 35.0).abs() < EPS);
    assert!((lines[1].items[0].x - 14.0).abs() < EPS);
    for line in lines {
        let right = line
            .items
            .iter()
            .filter(|i| matches!(i.kind, PlacedKind::Text(_)))
            .map(|i| i.x + i.width)
            .fold(0.0, f32::max);
        assert!(right <= 126.0 + EPS);
    }
}

#[test]
fn a_hanging_indent_pulls_the_first_line_left() {
    let style = ParaStyle {
        indent_left: Dip(28.0),
        indent_first: Dip(-14.0),
        ..ParaStyle::default()
    };
    let doc = with_style(style, |_| {
        vec![Run::Text("hello world", CharStyleId::DEFAULT)]
    });
    let layout = lay(&doc, 400.0);
    assert!((layout.paragraphs()[0].lines[0].items[0].x - 14.0).abs() < EPS);
}

#[test]
fn headings_are_larger_and_quotes_are_indented_and_ruled() {
    let heading = with_style(
        ParaStyle {
            kind: BlockKind::Heading(1),
            ..ParaStyle::default()
        },
        |_| vec![Run::Text("Title", CharStyleId::DEFAULT)],
    );
    let layout = lay(&heading, 400.0);
    let line = &layout.paragraphs()[0].lines[0];
    assert!((line.items[0].width - 5.0 * 7.0 * 1.8).abs() < EPS);
    assert!(line.height >= 31.0);

    let quote = with_style(
        ParaStyle {
            kind: BlockKind::Quote,
            ..ParaStyle::default()
        },
        |_| vec![Run::Text("Quoted", CharStyleId::DEFAULT)],
    );
    let layout = lay(&quote, 400.0);
    let p = &layout.paragraphs()[0];
    assert!((p.lines[0].items[0].x - 24.0).abs() < EPS);
    let (x, top, bottom) = p.rule.expect("a quote has a rule");
    assert!(x < 24.0 && bottom > top);
}

fn list_doc(items: &[(ListKind, u8)]) -> (Document, Layout) {
    let mut b = DocBuilder::new();
    for &(kind, level) in items {
        let id = b.para_style(ParaStyle {
            list: Some(ListItem { kind, level }),
            ..ParaStyle::default()
        });
        b.paragraph(id, &[Run::Text("item", CharStyleId::DEFAULT)]);
    }
    let doc = b.finish();
    let layout = lay(&doc, 400.0);
    (doc, layout)
}

/// The marker's width in characters.
fn marker_chars(layout: &Layout, index: usize) -> usize {
    (layout.paragraphs()[index]
        .marker
        .as_ref()
        .unwrap()
        .layout
        .width()
        / 7.0)
        .round() as usize
}

#[test]
fn numbers_count_consecutive_items_of_a_level_across_nested_ones() {
    use ListKind::Numbered;
    let mut items = vec![(Numbered, 0); 8];
    items.push((Numbered, 1));
    items.push((Numbered, 0));
    items.push((Numbered, 0));
    let (_, layout) = list_doc(&items);
    assert_eq!(marker_chars(&layout, 7), 2, "8. is two characters");
    assert_eq!(
        marker_chars(&layout, 8),
        2,
        "the nested item restarts at 1."
    );
    assert_eq!(marker_chars(&layout, 9), 2, "9. after the nested item");
    assert_eq!(
        marker_chars(&layout, 10),
        3,
        "10. continues the outer count"
    );
}

#[test]
fn a_bullet_or_plain_paragraph_restarts_numbering() {
    use ListKind::{Bullet, Numbered};
    let mut items = vec![(Numbered, 0); 9];
    items.push((Bullet, 0));
    items.push((Numbered, 0));
    let (_, layout) = list_doc(&items);
    assert_eq!(marker_chars(&layout, 10), 2, "1. after a bullet");
    assert_eq!(marker_chars(&layout, 9), 1, "a bullet is one character");
}

#[test]
fn list_text_is_indented_by_level_and_the_marker_hangs_left_of_it() {
    use ListKind::Bullet;
    let (_, layout) = list_doc(&[(Bullet, 0), (Bullet, 1)]);
    let text_x = |i: usize| layout.paragraphs()[i].lines[0].items[0].x;
    assert!((text_x(0) - 24.0).abs() < EPS);
    assert!((text_x(1) - 48.0).abs() < EPS);
    for i in 0..2 {
        let m = layout.paragraphs()[i].marker.as_ref().unwrap();
        assert!(m.x + m.layout.width() < text_x(i));
    }
}
