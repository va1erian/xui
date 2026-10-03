#![forbid(unsafe_code)]

//! Line breaking, alignment, justification and baselines.

use xui_core::{Color, Dip};

use super::*;
use crate::layout::PlacedKind;
use crate::model::{Align, Baseline, CharStyle, InlineImage, ParaStyle, Wrap};

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
            image: gradient_image(20, 40, Color::rgb(0, 0, 0), Color::rgb(9, 9, 9)),
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
