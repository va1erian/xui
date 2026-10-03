//! Values from outside the document (a loader, a caller-built inverse op) are
//! checked before they can break an invariant.

#[path = "model_common.rs"]
mod common;

use common::*;
use xui_rich_text::model::{
    Align, CharStyle, CharStyleId, DocPos, Document, EditError, EditOp, ParaStyle, Span,
};

fn with_image() -> Document {
    let mut doc = Document::from_plain_text("ab");
    apply(
        &mut doc,
        EditOp::InsertObject {
            at: DocPos::new(0, 1),
            object: object(1),
        },
    );
    doc
}

#[test]
fn an_object_anchored_twice_is_rejected() {
    let doc = with_image();
    let para = doc.paragraphs()[0].clone();
    let result = Document::from_parts(
        vec![para.clone(), para],
        doc.styles().clone(),
        doc.objects().clone(),
    );
    assert!(result.unwrap_err().contains("anchored twice"));
}

#[test]
fn restored_spans_must_cover_the_text() {
    let mut doc = Document::from_plain_text("hello");
    let before = doc.clone();
    let spans = vec![Span {
        len: 99,
        style: CharStyleId::DEFAULT,
    }];
    let result = doc.apply(EditOp::RestoreSpans(vec![(0, spans)]));
    assert!(matches!(result, Err(EditError::BadParagraphs(_))));
    assert!(doc.content_eq(&before), "the document is unchanged");
}

#[test]
fn restored_styles_must_belong_to_the_document() {
    let mut other = Document::new();
    let foreign_char = other.styles_mut().intern_char(CharStyle {
        italic: true,
        ..CharStyle::default()
    });
    let foreign_para = other.styles_mut().intern_para(ParaStyle {
        align: Align::Center,
        ..ParaStyle::default()
    });

    let mut doc = Document::from_plain_text("hello");
    let spans = vec![Span {
        len: 5,
        style: foreign_char,
    }];
    let result = doc.apply(EditOp::RestoreSpans(vec![(0, spans)]));
    assert!(matches!(result, Err(EditError::UnknownStyle)));
    let result = doc.apply(EditOp::RestoreParaStyles(vec![(0, foreign_para)]));
    assert!(matches!(result, Err(EditError::UnknownStyle)));
    doc.check().unwrap();
}
