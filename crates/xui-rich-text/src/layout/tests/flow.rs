#![forbid(unsafe_code)]

//! The document flow and incremental relayout.

use std::sync::Arc;

use xui_core::{Color, Dip};

use super::*;
use crate::layout::PlacedKind;
use crate::model::{DocPos, DocRange, EditOp, InlineImage, ParaStyleId, Side, Wrap};

const WORDS: &str = "aaaa bbbb cccc dddd eeee ffff gggg hhhh iiii jjjj";

/// The shaped layout behind the first text item of paragraph `index`.
fn first_word(layout: &Layout, index: usize) -> usize {
    match &layout.paragraphs()[index].lines[0].items[0].kind {
        PlacedKind::Text(t) => Arc::as_ptr(t).cast::<()>() as usize,
        _ => panic!("a word"),
    }
}

fn set_text(doc: &mut Document, index: usize, text: &str) {
    let end = DocPos::new(index, doc.paragraphs()[index].text().len());
    let range = DocRange::new(DocPos::new(index, 0), end);
    doc.apply(EditOp::Delete { range }).unwrap();
    insert(doc, DocPos::new(index, 0), text);
}

fn insert(doc: &mut Document, at: DocPos, text: &str) {
    let op = EditOp::InsertText {
        at,
        text: text.into(),
        style: None,
    };
    doc.apply(op).unwrap();
}

fn tops(layout: &Layout) -> Vec<f32> {
    layout.paragraphs().iter().map(|p| p.y).collect()
}

#[test]
fn paragraphs_stack_top_to_bottom() {
    let doc = plain(&["one", "two", "three"]);
    let layout = lay(&doc, 200.0);
    assert_eq!(tops(&layout), [0.0, 18.0, 36.0]);
    assert!((layout.height() - 54.0).abs() < 0.01);
}

#[test]
fn editing_one_paragraph_leaves_the_others_untouched() {
    let mut doc = plain(&[WORDS, WORDS, WORDS, WORDS]);
    let mut layout = lay(&doc, 140.0);
    let before: Vec<_> = (0..4).map(|i| first_word(&layout, i)).collect();
    let tops_before = tops(&layout);

    set_text(
        &mut doc,
        1,
        "xxxx bbbb cccc dddd eeee ffff gggg hhhh iiii jjjj",
    );
    layout.mark_dirty(1);
    assert_eq!(layout.update(&doc, &Mono), Some(1));

    assert_eq!(first_word(&layout, 0), before[0]);
    assert_ne!(
        first_word(&layout, 1),
        before[1],
        "the edited paragraph was shaped again"
    );
    assert_eq!(first_word(&layout, 2), before[2], "the next one was reused");
    assert_eq!(first_word(&layout, 3), before[3]);
    assert_eq!(tops(&layout), tops_before, "same height, nothing moves");
    assert_eq!(layout.update(&doc, &Mono), None, "nothing is dirty now");
}

#[test]
fn a_taller_paragraph_moves_the_ones_below_without_relaying_them_out() {
    let mut doc = plain(&["a", WORDS, "tail"]);
    let mut layout = lay(&doc, 140.0);
    let tail = first_word(&layout, 2);
    let old = tops(&layout)[2];
    set_text(&mut doc, 1, &format!("{WORDS} {WORDS}"));
    layout.mark_dirty(1);
    layout.update(&doc, &Mono);
    assert!(tops(&layout)[2] > old);
    assert_eq!(first_word(&layout, 2), tail);
}

#[test]
fn removing_a_float_relays_out_the_paragraphs_it_pushed_text_around() {
    let mut b = DocBuilder::new();
    let img = b.object(InlineImage {
        image: gradient_image(8, 8, Color::rgb(0, 0, 0), Color::rgb(9, 9, 9)),
        size: (Dip(60.0), Dip(200.0)),
        wrap: Wrap::square(Side::Left),
        alt: String::new(),
    });
    b.paragraph(
        ParaStyleId::DEFAULT,
        &[Run::Object(img), Run::Text("x", CharStyleId::DEFAULT)],
    );
    b.paragraph(
        ParaStyleId::DEFAULT,
        &[Run::Text(WORDS, CharStyleId::DEFAULT)],
    );
    let mut doc = b.finish();
    let mut layout = lay(&doc, 300.0);
    assert!(
        layout.paragraphs()[1].lines[0].items[0].x >= 60.0,
        "pushed by the float"
    );

    set_text(&mut doc, 0, "x");
    layout.mark_dirty(0);
    layout.update(&doc, &Mono);
    assert!(
        layout.paragraphs()[1].lines[0].items[0].x.abs() < 0.01,
        "free again"
    );
}

#[test]
fn splicing_in_a_paragraph_matches_a_fresh_layout() {
    let mut doc = plain(&[WORDS, "middle", WORDS]);
    let mut layout = lay(&doc, 140.0);
    let end = DocPos::new(0, doc.paragraphs()[0].text().len());
    doc.apply(EditOp::SplitParagraph { at: end }).unwrap();
    insert(&mut doc, DocPos::new(1, 0), "new paragraph here");
    layout.splice(1, 0, 1);
    layout.update(&doc, &Mono);
    let fresh = lay(&doc, 140.0);
    assert_eq!(tops(&layout), tops(&fresh));
    for i in 0..doc.paragraphs().len() {
        assert_eq!(line_texts(&doc, &layout, i), line_texts(&doc, &fresh, i));
    }
}

#[test]
fn a_new_width_relays_everything_out() {
    let doc = plain(&[WORDS]);
    let mut layout = lay(&doc, 300.0);
    let wide = layout.paragraphs()[0].lines.len();
    assert!(
        !layout.set_metrics(300.0, 96),
        "same metrics change nothing"
    );
    assert!(layout.set_metrics(100.0, 96));
    layout.update(&doc, &Mono);
    assert!(layout.paragraphs()[0].lines.len() > wide);
}

#[test]
fn a_higher_dpi_scales_the_layout() {
    let doc = plain(&["hello"]);
    let mut layout = Layout::new();
    layout.set_metrics(400.0, 192);
    layout.update(&doc, &Mono);
    let word = &layout.paragraphs()[0].lines[0].items[0];
    assert!(
        (word.width - 5.0 * 14.0).abs() < 0.01,
        "twice as wide at 192 dpi"
    );
}

#[test]
fn visible_paragraphs_are_found_by_their_vertical_span() {
    let doc = plain(&["a", "b", "c", "d", "e"]);
    let layout = lay(&doc, 200.0);
    assert_eq!(layout.visible(0.0, 1.0), 0..1);
    assert_eq!(layout.visible(20.0, 60.0), 1..4);
    assert_eq!(layout.visible(0.0, 1000.0), 0..5);
}
