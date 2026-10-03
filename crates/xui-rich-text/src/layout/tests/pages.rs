#![forbid(unsafe_code)]

//! Pagination: lines and floats kept off the page gaps, page breaks, and
//! incremental and viewport-first layout agreeing with a full one on pages.

use xui_core::{Color, Dip};

use super::*;
use crate::layout::Pages;
use crate::model::{
    CharStyle, DocPos, EditOp, InlineImage, ParaStyleId, ParaStylePatch, Side, Wrap,
};

const SENTENCE: &str = "the quick brown fox jumps over the lazy dog and keeps running far away";

/// Pages of 100 px of text, 150 px apart: five 18 px lines to a page.
fn pages() -> Pages {
    Pages::new(100.0, 150.0).unwrap()
}

fn lay_paged(doc: &Document, width: f32) -> Layout {
    let mut layout = Layout::new();
    layout.set_metrics(width, 96);
    layout.set_pages(Some(pages()));
    layout.update(doc, &Mono);
    layout
}

fn many(count: usize) -> Document {
    let mut b = DocBuilder::new();
    for i in 0..count {
        let text = format!("{i}: {SENTENCE}");
        b.paragraph(
            ParaStyleId::DEFAULT,
            &[Run::Text(&text, CharStyleId::DEFAULT)],
        );
    }
    b.finish()
}

/// Every line's top and bottom in flow pixels.
fn line_spans(layout: &Layout) -> Vec<(f32, f32)> {
    layout
        .paragraphs()
        .iter()
        .flat_map(|p| {
            p.lines
                .iter()
                .map(move |l| (p.y + l.y, p.y + l.y + l.height))
        })
        .collect()
}

fn assert_on_pages(layout: &Layout) {
    let p = pages();
    for (top, bottom) in line_spans(layout) {
        assert!(
            p.fits(top, bottom - top),
            "a line at {top}..{bottom} crosses a page edge"
        );
    }
}

fn assert_same(a: &Layout, b: &Layout, doc: &Document) {
    assert!((a.height() - b.height()).abs() < 0.01, "heights differ");
    for i in 0..doc.paragraphs().len() {
        assert_eq!(a.paragraphs()[i].y, b.paragraphs()[i].y, "paragraph {i}");
        let (la, lb) = (&a.paragraphs()[i].lines, &b.paragraphs()[i].lines);
        assert_eq!(
            la.iter().map(|l| l.y).collect::<Vec<_>>(),
            lb.iter().map(|l| l.y).collect::<Vec<_>>(),
            "paragraph {i}"
        );
        assert_eq!(
            line_texts(doc, a, i),
            line_texts(doc, b, i),
            "paragraph {i}"
        );
    }
}

fn picture(w: f32, h: f32, wrap: Wrap) -> InlineImage {
    InlineImage {
        image: gradient_image(8, 8, Color::rgb(0, 0, 0), Color::rgb(9, 9, 9)),
        size: (Dip(w), Dip(h)),
        wrap,
        alt: String::new(),
    }
}

#[test]
fn lines_never_cross_a_page_edge() {
    let doc = many(20);
    let layout = lay_paged(&doc, 200.0);
    assert_on_pages(&layout);
    assert!(layout.page_count() > 5, "{} pages", layout.page_count());
    assert_eq!(
        layout.page_count(),
        layout.page_at(layout.height() - 1.0) + 1
    );
}

#[test]
fn a_long_paragraph_continues_at_the_next_page_top() {
    let text = [SENTENCE; 3].join(" ");
    let doc = plain(&[&text]);
    let layout = lay_paged(&doc, 140.0);
    let spans = line_spans(&layout);
    assert!(spans.len() > 5);
    // Five lines fill the first page; the sixth starts the second.
    assert_eq!(spans[4], (72.0, 90.0));
    assert_eq!(spans[5].0, 150.0);
    assert_on_pages(&layout);
}

#[test]
fn empty_paragraphs_never_sit_in_a_gap() {
    let doc = plain(&[""; 12]);
    let layout = lay_paged(&doc, 200.0);
    assert_on_pages(&layout);
    assert_eq!(
        layout.paragraphs()[5].lines[0].y + layout.paragraphs()[5].y,
        150.0
    );
}

#[test]
fn page_break_before_starts_the_next_page() {
    let mut doc = plain(&["one", "two", "three"]);
    doc.apply(EditOp::SetParaStyle {
        paras: 1..2,
        patch: ParaStylePatch::page_break_before(true),
    })
    .unwrap();
    let layout = lay_paged(&doc, 200.0);
    let p = &layout.paragraphs()[1];
    assert_eq!(p.y + p.lines[0].y, 150.0);
    assert_eq!(layout.paragraphs()[2].y, 168.0);
    assert_eq!(layout.page_count(), 2);
    // Off pages the break does nothing.
    let flat = lay(&doc, 200.0);
    assert_eq!(flat.paragraphs()[1].y, 18.0);
}

#[test]
fn a_page_break_at_a_page_top_adds_no_page() {
    let mut doc = plain(&["a", "b", "c", "d", "e", "f"]);
    doc.apply(EditOp::SetParaStyle {
        paras: 5..6,
        patch: ParaStylePatch::page_break_before(true),
    })
    .unwrap();
    let layout = lay_paged(&doc, 200.0);
    // "f" lands on page 2 by itself (five lines fill page 1); the break
    // keeps it there instead of pushing it to page 3.
    let p = &layout.paragraphs()[5];
    assert_eq!(p.y + p.lines[0].y, 150.0);
    assert_eq!(layout.page_count(), 2);
}

#[test]
fn a_float_that_does_not_fit_moves_to_the_next_page() {
    let mut doc = plain(&["a", "b", "c", "d", SENTENCE]);
    doc.apply(EditOp::InsertObject {
        at: DocPos::new(4, 0),
        object: picture(40.0, 60.0, Wrap::square(Side::Left)),
    })
    .unwrap();
    let layout = lay_paged(&doc, 200.0);
    let p = &layout.paragraphs()[4];
    let rect = p.floats[0].rect.shifted(p.y);
    assert_eq!(rect.top, 150.0);
    assert_on_pages(&layout);
    assert_clear_of_floats(&layout);
}

#[test]
fn a_top_and_bottom_picture_moves_to_the_next_page() {
    let mut doc = plain(&["a", "b", "c", "d"]);
    doc.apply(EditOp::InsertObject {
        at: DocPos::new(3, 1),
        object: picture(40.0, 60.0, Wrap::TopAndBottom { margin: Dip(4.0) }),
    })
    .unwrap();
    let layout = lay_paged(&doc, 200.0);
    let p = &layout.paragraphs()[3];
    let rect = p.floats[0].rect.shifted(p.y);
    assert!(rect.top >= 150.0, "{rect:?}");
    assert_on_pages(&layout);
}

#[test]
fn a_float_taller_than_a_page_is_shrunk_to_fit_it() {
    let mut doc = plain(&["a", SENTENCE]);
    doc.apply(EditOp::InsertObject {
        at: DocPos::new(1, 0),
        object: picture(100.0, 400.0, Wrap::square(Side::Right)),
    })
    .unwrap();
    let layout = lay_paged(&doc, 200.0);
    let p = &layout.paragraphs()[1];
    let rect = p.floats[0].rect.shifted(p.y);
    assert_eq!(rect.top, 150.0);
    assert!((rect.height() - 100.0).abs() < 0.01, "{rect:?}");
    assert!((rect.width() - 25.0).abs() < 0.01, "{rect:?}");
}

#[test]
fn a_line_taller_than_a_page_overflows_instead_of_looping() {
    let mut b = DocBuilder::new();
    let big = b.char_style(CharStyle {
        size: Dip(200.0),
        ..CharStyle::default()
    });
    b.paragraph(
        ParaStyleId::DEFAULT,
        &[Run::Text("x", CharStyleId::DEFAULT)],
    );
    b.paragraph(ParaStyleId::DEFAULT, &[Run::Text("W", big)]);
    b.paragraph(
        ParaStyleId::DEFAULT,
        &[Run::Text("after", CharStyleId::DEFAULT)],
    );
    let doc = b.finish();
    let layout = lay_paged(&doc, 200.0);
    let p = &layout.paragraphs()[1];
    assert_eq!(p.y + p.lines[0].y, 150.0, "the tall line starts a page");
    let after = &layout.paragraphs()[2];
    assert_eq!(
        after.y + after.lines[0].y,
        450.0,
        "and the text after it the next free page"
    );
}

#[test]
fn an_edit_above_a_page_edge_matches_a_full_layout() {
    let mut doc = many(30);
    let mut layout = lay_paged(&doc, 200.0);
    for (para, text) in [(2, " more words to wrap"), (0, " and more"), (7, " x")] {
        let at = DocPos::new(para, doc.paragraphs()[para].text().len());
        doc.apply(EditOp::InsertText {
            at,
            text: text.repeat(3),
            style: None,
        })
        .unwrap();
        layout.mark_dirty(para);
        layout.update(&doc, &Mono);
        assert_same(&layout, &lay_paged(&doc, 200.0), &doc);
        assert_on_pages(&layout);
    }
}

#[test]
fn paragraphs_wholly_inside_a_page_are_reused_after_an_edit_above() {
    let mut doc = many(30);
    let mut layout = lay_paged(&doc, 400.0);
    let at = DocPos::new(0, doc.paragraphs()[0].text().len());
    doc.apply(EditOp::InsertText {
        at,
        text: format!(" {SENTENCE}"),
        style: None,
    })
    .unwrap();
    layout.mark_dirty(0);
    let first = layout.update(&doc, &Mono);
    assert_eq!(first, Some(0));
    assert_same(&layout, &lay_paged(&doc, 400.0), &doc);
}

#[test]
fn viewport_first_layout_on_pages_converges_to_a_full_one() {
    let doc = many(300);
    let mut partial = Layout::new();
    partial.set_metrics(200.0, 96);
    partial.set_pages(Some(pages()));
    partial.update_around(&doc, &Mono, 0.0, 1.0, 1);
    let y = partial.paragraphs()[250].y;
    partial.update_around(&doc, &Mono, y, y + 300.0, 32);
    let mut slices = 0;
    while partial.update_idle(&doc, &Mono, 16) {
        slices += 1;
        assert!(slices < 200, "the idle pass must finish");
    }
    assert_same(&partial, &lay_paged(&doc, 200.0), &doc);
}

#[test]
fn turning_pages_off_restores_the_continuous_flow() {
    let doc = many(20);
    let mut layout = lay_paged(&doc, 200.0);
    assert!(layout.set_pages(None));
    assert!(!layout.set_pages(None));
    layout.update(&doc, &Mono);
    assert_same(&layout, &lay(&doc, 200.0), &doc);
    assert_eq!(layout.page_count(), 1);
}
