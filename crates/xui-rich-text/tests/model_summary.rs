//! The style summary a toolbar reads.

#[path = "model_common.rs"]
mod common;

use common::{apply, bold, range};
use xui_core::Dip;
use xui_rich_text::model::{
    Align, BlockKind, CharStylePatch, DocPos, Document, EditOp, ParaStylePatch, Selection, Tri,
};

#[test]
fn bold_is_uniform_or_mixed() {
    let mut doc = Document::from_plain_text("abc def");
    bold(&mut doc, range(0, 0, 0, 3));
    let whole = Selection::text(DocPos::new(0, 0), DocPos::new(0, 7));
    assert_eq!(doc.style_summary(&whole).bold, Tri::Mixed);
    let word = Selection::text(DocPos::new(0, 0), DocPos::new(0, 3));
    assert_eq!(doc.style_summary(&word).bold, Tri::Uniform(true));
    let rest = Selection::text(DocPos::new(0, 4), DocPos::new(0, 7));
    assert_eq!(doc.style_summary(&rest).bold, Tri::Uniform(false));
    assert_eq!(doc.style_summary(&word).italic, Tri::Uniform(false));
}

#[test]
fn a_caret_reports_the_typing_style() {
    let mut doc = Document::from_plain_text("abc def");
    bold(&mut doc, range(0, 0, 0, 3));
    let summary = |byte| doc.style_summary(&Selection::caret(DocPos::new(0, byte)));
    assert_eq!(summary(3).bold, Tri::Uniform(true));
    assert_eq!(summary(4).bold, Tri::Uniform(false));
    assert_eq!(summary(0).bold, Tri::Uniform(true));
    let empty = Document::new();
    let summary = empty.style_summary(&Selection::caret(DocPos::default()));
    assert_eq!(summary.size, Tri::Uniform(Dip(14.0)));
    assert_eq!(summary.align, Tri::Uniform(Align::Left));
    assert_eq!(summary.kind, Tri::Uniform(BlockKind::Body));
}

#[test]
fn paragraph_attributes_follow_the_paragraphs_touched() {
    let mut doc = Document::from_plain_text("a\nb\nc");
    apply(
        &mut doc,
        EditOp::SetParaStyle {
            paras: 1..2,
            patch: ParaStylePatch::align(Align::Center),
        },
    );
    let sel = |a, b| Selection::text(DocPos::new(a, 0), DocPos::new(b, 1));
    assert_eq!(doc.style_summary(&sel(0, 2)).align, Tri::Mixed);
    assert_eq!(
        doc.style_summary(&sel(1, 1)).align,
        Tri::Uniform(Align::Center)
    );
    let to_start = Selection::text(DocPos::new(0, 0), DocPos::new(1, 0));
    assert_eq!(
        doc.style_summary(&to_start).align,
        Tri::Uniform(Align::Left)
    );
}

#[test]
fn size_and_family_vary_independently() {
    let mut doc = Document::from_plain_text("abcd");
    apply(
        &mut doc,
        EditOp::SetCharStyle {
            range: range(0, 2, 0, 4),
            patch: CharStylePatch {
                size: Some(Dip(20.0)),
                family: Some(Some("Serif".into())),
                ..CharStylePatch::default()
            },
        },
    );
    let summary = doc.style_summary(&Selection::text(DocPos::new(0, 0), DocPos::new(0, 4)));
    assert_eq!(summary.size, Tri::Mixed);
    assert_eq!(summary.family, Tri::Mixed);
    let tail = doc.style_summary(&Selection::text(DocPos::new(0, 2), DocPos::new(0, 4)));
    assert_eq!(tail.size, Tri::Uniform(Dip(20.0)));
    assert_eq!(tail.family, Tri::Uniform(Some("Serif".to_owned())));
}
