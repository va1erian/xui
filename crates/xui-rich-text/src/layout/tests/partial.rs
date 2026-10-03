#![forbid(unsafe_code)]

//! Viewport-first layout: a bounded first pass, estimates, and the in-order
//! finish that must agree with a full layout.

use xui_core::{Color, Dip};

use super::*;
use crate::model::{InlineImage, ParaStyleId, Side, Wrap};

const SENTENCE: &str = "the quick brown fox jumps over the lazy dog and keeps running far away";

fn many(count: usize) -> Document {
    let mut b = DocBuilder::new();
    for i in 0..count {
        let text = format!("{i}: {SENTENCE} {SENTENCE}");
        b.paragraph(
            ParaStyleId::DEFAULT,
            &[Run::Text(&text, CharStyleId::DEFAULT)],
        );
    }
    b.finish()
}

fn laid(layout: &Layout) -> usize {
    layout.paragraphs().iter().filter(|p| !p.dirty).count()
}

#[test]
fn the_first_pass_lays_out_only_the_window() {
    let doc = many(2000);
    let mut layout = Layout::new();
    layout.set_metrics(300.0, 96);
    layout.update_around(&doc, &Mono, 0.0, 600.0, 64);
    let done = laid(&layout);
    assert!((1..=64).contains(&done), "{done} paragraphs laid out");
    assert!(!layout.is_complete());
    assert!(
        layout.height() > 2000.0 * 18.0,
        "estimated heights fill the rest"
    );
    assert!(
        layout.paragraphs()[0].bottom() > 0.0 && !layout.paragraphs()[0].lines.is_empty(),
        "the top is real"
    );
}

#[test]
fn a_window_in_the_middle_is_laid_out_where_the_estimates_put_it() {
    let doc = many(2000);
    let mut layout = Layout::new();
    layout.set_metrics(300.0, 96);
    layout.update_around(&doc, &Mono, 0.0, 1.0, 1);
    let y = layout.paragraphs()[1000].y;
    layout.update_around(&doc, &Mono, y, y + 400.0, 64);
    assert!(!layout.paragraphs()[1000].lines.is_empty());
    assert!(laid(&layout) < 100);
}

#[test]
fn finishing_in_slices_matches_a_full_layout() {
    let mut doc = many(300);
    // A tall float early on pushes the next paragraphs' text aside.
    let image = InlineImage {
        image: gradient_image(8, 8, Color::rgb(0, 0, 0), Color::rgb(9, 9, 9)),
        size: (Dip(60.0), Dip(400.0)),
        wrap: Wrap::square(Side::Left),
        alt: String::new(),
    };
    doc.apply(crate::model::EditOp::InsertObject {
        at: crate::model::DocPos::new(2, 0),
        object: image,
    })
    .unwrap();

    let mut partial = Layout::new();
    partial.set_metrics(300.0, 96);
    // Start from the far end, so paragraphs there are laid out speculatively.
    partial.update_around(&doc, &Mono, 0.0, 1.0, 1);
    let y = partial.paragraphs()[250].y;
    partial.update_around(&doc, &Mono, y, y + 300.0, 32);
    let mut slices = 0;
    while partial.update_idle(&doc, &Mono, 16) {
        slices += 1;
        assert!(slices < 100, "the idle pass must finish");
    }
    assert!(partial.is_complete());

    let full = lay(&doc, 300.0);
    assert!((partial.height() - full.height()).abs() < 0.01);
    for i in 0..doc.paragraphs().len() {
        assert_eq!(
            partial.paragraphs()[i].y,
            full.paragraphs()[i].y,
            "paragraph {i}"
        );
        assert_eq!(
            line_texts(&doc, &partial, i),
            line_texts(&doc, &full, i),
            "paragraph {i}"
        );
    }
}

#[test]
fn a_new_width_estimates_again_and_finishes_the_same() {
    let doc = many(400);
    let mut layout = lay(&doc, 600.0);
    layout.set_metrics(250.0, 96);
    layout.update_around(&doc, &Mono, 0.0, 400.0, 64);
    assert!(!layout.is_complete());
    while layout.update_idle(&doc, &Mono, 50) {}
    let full = lay(&doc, 250.0);
    assert!((layout.height() - full.height()).abs() < 0.01);
}

#[test]
fn removing_a_float_in_the_window_revisits_the_clean_paragraphs_beside_it() {
    use crate::model::{DocPos, DocRange, EditOp};
    let mut doc = many(20);
    let image = InlineImage {
        image: gradient_image(8, 8, Color::rgb(0, 0, 0), Color::rgb(9, 9, 9)),
        size: (Dip(60.0), Dip(400.0)),
        wrap: Wrap::square(Side::Left),
        alt: String::new(),
    };
    doc.apply(EditOp::InsertObject {
        at: DocPos::new(0, 0),
        object: image,
    })
    .unwrap();
    let mut layout = lay(&doc, 300.0);

    // Remove the float; only paragraph 0 is dirty and only it fits the budget.
    let range = DocRange::new(DocPos::new(0, 0), DocPos::new(0, 3));
    doc.apply(EditOp::Delete { range }).unwrap();
    layout.mark_dirty(0);
    layout.update_around(&doc, &Mono, 0.0, 1.0, 1);
    assert!(
        !layout.is_complete(),
        "paragraph 1 still wraps around the removed float"
    );
    while layout.update_idle(&doc, &Mono, 4) {}

    let full = lay(&doc, 300.0);
    for i in 0..doc.paragraphs().len() {
        assert_eq!(
            line_texts(&doc, &layout, i),
            line_texts(&doc, &full, i),
            "paragraph {i}"
        );
    }
}
