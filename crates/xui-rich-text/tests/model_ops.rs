//! Edit operations: edge cases and exact inverses.

#[path = "model_common.rs"]
mod common;

use common::*;
use xui_rich_text::model::{
    Align, BlockKind, CharStylePatch, DocPos, Document, EditError, EditOp, OBJECT_CHAR,
    ParaStylePatch, Selection,
};

fn doc(text: &str) -> Document {
    Document::from_plain_text(text)
}

fn insert(doc: &mut Document, para: usize, byte: usize, text: &str) -> EditOp {
    apply(
        doc,
        EditOp::InsertText {
            at: DocPos::new(para, byte),
            text: text.into(),
            style: None,
        },
    )
}

/// Applies `op`, checks the document, then undoes it and compares.
fn round_trip(d: &mut Document, op: EditOp) {
    let before = d.clone();
    let inverse = apply(d, op);
    d.check().unwrap();
    let redo = apply(d, inverse);
    d.check().unwrap();
    assert!(d.content_eq(&before), "inverse restores the document");
    let _ = redo;
}

#[test]
fn insert_takes_the_style_before_the_caret() {
    let mut d = doc("ab");
    bold(&mut d, range(0, 0, 0, 2));
    insert(&mut d, 0, 2, "c");
    assert_eq!(runs(&d, 0), vec![("abc".to_owned(), true)]);
    insert(&mut d, 0, 0, "z");
    assert_eq!(runs(&d, 0), vec![("zabc".to_owned(), true)]);
}

#[test]
fn insert_with_newlines_makes_paragraphs() {
    let mut d = doc("ad");
    let inverse = insert(&mut d, 0, 1, "b\r\nc\n");
    assert_eq!(d.to_plain_text(), "ab\nc\nd");
    d.check().unwrap();
    apply(&mut d, inverse);
    assert_eq!(d.to_plain_text(), "ad");
}

#[test]
fn insert_drops_object_chars() {
    let mut d = doc("");
    insert(&mut d, 0, 0, &format!("a{OBJECT_CHAR}b"));
    assert_eq!(d.to_plain_text(), "ab");
    d.check().unwrap();
}

#[test]
fn delete_across_paragraphs_with_mixed_styles() {
    let mut d = doc("abc\ndef\nghi");
    bold(&mut d, range(0, 1, 1, 2));
    bold(&mut d, range(2, 0, 2, 2));
    d.apply(EditOp::SetParaStyle {
        paras: 2..3,
        patch: ParaStylePatch::kind(BlockKind::Quote),
    })
    .unwrap();
    let before = d.clone();
    let inverse = apply(
        &mut d,
        EditOp::Delete {
            range: range(0, 2, 2, 1),
        },
    );
    assert_eq!(d.to_plain_text(), "abhi");
    assert_eq!(
        runs(&d, 0),
        vec![
            ("a".to_owned(), false),
            ("bh".to_owned(), true),
            ("i".to_owned(), false)
        ]
    );
    d.check().unwrap();
    apply(&mut d, inverse);
    assert!(d.content_eq(&before));
}

#[test]
fn delete_an_object_and_undo_restores_the_same_id() {
    let mut d = doc("ab");
    insert_object(&mut d);
    let id = d.paragraphs()[0].anchors()[0];
    let pos = d.object_pos(id).unwrap();
    let del = EditOp::Delete {
        range: d.selection_range(&Selection::Object(id)).unwrap(),
    };
    let inverse = apply(&mut d, del);
    assert!(d.objects().get(id).is_none());
    assert_eq!(d.to_plain_text(), "ab");
    d.check().unwrap();
    apply(&mut d, inverse);
    assert_eq!(d.paragraphs()[0].anchors(), &[id]);
    assert_eq!(d.object_pos(id), Some(pos));
    assert!(d.objects().get(id).is_some());
    d.check().unwrap();
}

fn insert_object(d: &mut Document) {
    apply(
        d,
        EditOp::InsertObject {
            at: DocPos::new(0, 1),
            object: object(3),
        },
    );
}

#[test]
fn split_and_merge_at_start_end_and_inside() {
    for at in [0, 2, 4] {
        let mut d = doc("abcd");
        bold(&mut d, range(0, 0, 0, 2));
        round_trip(
            &mut d,
            EditOp::SplitParagraph {
                at: DocPos::new(0, at),
            },
        );
        let mut d = doc("abcd\nef");
        bold(&mut d, range(0, 2, 1, 1));
        round_trip(&mut d, EditOp::MergeParagraph { para: 0 });
    }
    let mut d = doc("abcd");
    apply(
        &mut d,
        EditOp::SplitParagraph {
            at: DocPos::new(0, 1),
        },
    );
    assert_eq!(d.to_plain_text(), "a\nbcd");
}

#[test]
fn empty_paragraph_keeps_its_style() {
    let mut d = doc("ab");
    bold(&mut d, range(0, 0, 0, 2));
    d.apply(EditOp::SetParaStyle {
        paras: 0..1,
        patch: ParaStylePatch::align(Align::Center),
    })
    .unwrap();
    let inverse = apply(
        &mut d,
        EditOp::Delete {
            range: range(0, 0, 0, 2),
        },
    );
    assert_eq!(d.paragraphs()[0].text(), "");
    assert!(runs(&d, 0)[0].1, "the emptied paragraph still types bold");
    insert(&mut d, 0, 0, "x");
    assert_eq!(runs(&d, 0), vec![("x".to_owned(), true)]);
    let _ = inverse;

    let mut d = doc("ab");
    bold(&mut d, range(0, 0, 0, 2));
    let split = apply(
        &mut d,
        EditOp::SplitParagraph {
            at: DocPos::new(0, 2),
        },
    );
    assert!(runs(&d, 1)[0].1, "Enter at the end keeps bold typing");
    let style = d.paragraphs()[1].style();
    assert_eq!(style, d.paragraphs()[0].style());
    apply(&mut d, split);
    assert_eq!(d.paragraph_count(), 1);
}

#[test]
fn merge_restores_the_second_paragraph_style() {
    let mut d = doc("a\nb");
    d.apply(EditOp::SetParaStyle {
        paras: 1..2,
        patch: ParaStylePatch::kind(BlockKind::Heading(1)),
    })
    .unwrap();
    round_trip(&mut d, EditOp::MergeParagraph { para: 0 });
}

#[test]
fn multibyte_text_and_clusters() {
    let family = "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}";
    let mut d = doc(&format!("e\u{301}{family}z"));
    let end_of_e = "e\u{301}".len();
    round_trip(
        &mut d,
        EditOp::Delete {
            range: range(0, end_of_e, 0, end_of_e + family.len()),
        },
    );
    round_trip(
        &mut d,
        EditOp::SplitParagraph {
            at: DocPos::new(0, end_of_e),
        },
    );
    insert(&mut d, 0, end_of_e, family);
    assert_eq!(d.to_plain_text(), format!("e\u{301}{family}{family}z"));
    d.check().unwrap();
}

#[test]
fn bad_positions_are_errors_and_leave_the_document_alone() {
    let mut d = doc("é");
    let before = d.clone();
    for op in [
        EditOp::Delete {
            range: range(0, 0, 0, 1),
        },
        EditOp::Delete {
            range: range(0, 0, 3, 0),
        },
        EditOp::SplitParagraph {
            at: DocPos::new(0, 1),
        },
        EditOp::MergeParagraph { para: 0 },
        EditOp::SetParaStyle {
            paras: 0..5,
            patch: ParaStylePatch::default(),
        },
    ] {
        assert!(d.apply(op).is_err());
    }
    assert!(matches!(
        d.apply(EditOp::InsertText {
            at: DocPos::new(1, 0),
            text: "x".into(),
            style: None
        }),
        Err(EditError::BadPosition(_))
    ));
    assert!(d.content_eq(&before));
}

#[test]
fn char_style_normalizes_and_restores_exact_spans() {
    let mut d = doc("hello world");
    let before = d.clone();
    let make_bold = bold(&mut d, range(0, 0, 0, 5));
    bold(&mut d, range(0, 5, 0, 11));
    assert_eq!(runs(&d, 0), vec![("hello world".to_owned(), true)]);
    d.check().unwrap();
    apply(
        &mut d,
        EditOp::SetCharStyle {
            range: range(0, 0, 0, 11),
            patch: CharStylePatch::bold(false),
        },
    );
    assert!(d.content_eq(&before));
    assert_eq!(d.paragraphs()[0].spans().len(), 1);
    let _ = make_bold;
}

#[test]
fn restyling_a_selection_over_an_empty_paragraph_restyles_it() {
    let mut d = doc("a\n\nb");
    bold(&mut d, range(0, 0, 2, 1));
    assert!(runs(&d, 1)[0].1);
    assert!(d.paragraphs()[1].text().is_empty());
    d.check().unwrap();
}

#[test]
fn para_style_and_object_ops_invert() {
    let mut d = doc("a\nb\nc");
    round_trip(
        &mut d,
        EditOp::SetParaStyle {
            paras: 0..2,
            patch: ParaStylePatch::align(Align::Right),
        },
    );
    insert_object(&mut d);
    let id = d.paragraphs()[0].anchors()[0];
    round_trip(
        &mut d,
        EditOp::SetObject {
            id,
            object: object(8),
        },
    );
    let del = EditOp::Delete {
        range: d.selection_range(&Selection::Object(id)).unwrap(),
    };
    apply(&mut d, del);
    assert!(matches!(
        d.apply(EditOp::SetObject {
            id,
            object: object(2)
        }),
        Err(EditError::UnknownObject(_))
    ));
}

#[test]
fn helpers_read_the_document() {
    let mut d = doc("one\ntwo");
    insert_object(&mut d);
    assert_eq!(d.paragraph_count(), 2);
    assert_eq!(d.text_in(range(0, 1, 1, 1)).unwrap(), "ne\nt");
    let (inverses, caret) = d
        .insert_text(
            &Selection::text(DocPos::new(0, 0), DocPos::new(1, 1)),
            "x\ny",
        )
        .unwrap();
    assert_eq!(d.to_plain_text(), "x\nywo");
    assert_eq!(caret, DocPos::new(1, 1));
    assert!(d.objects().is_empty());
    for op in inverses.into_iter().rev() {
        d.apply(op).unwrap();
    }
    assert_eq!(d.to_plain_text(), "one\ntwo");
    assert_eq!(d.objects().len(), 1);
}

#[test]
fn fragments_round_trip_between_documents() {
    let mut src = doc("abc\ndef");
    bold(&mut src, range(0, 1, 1, 2));
    insert_object(&mut src);
    let fragment = src.extract_fragment(range(0, 0, 1, 3)).unwrap();
    assert_eq!(fragment.to_plain_text(), "abc\ndef");
    let mut dst = doc("xy");
    let (inverse, end) = dst.insert_fragment(DocPos::new(0, 1), &fragment).unwrap();
    dst.check().unwrap();
    assert_eq!(dst.to_plain_text(), "xabc\ndefy");
    assert_eq!(end, DocPos::new(1, 3));
    assert_eq!(dst.objects().len(), 1);
    assert_eq!(
        runs(&dst, 1),
        vec![("de".to_owned(), true), ("fy".to_owned(), false)]
    );
    dst.apply(inverse).unwrap();
    assert_eq!(dst.to_plain_text(), "xy");
    assert!(dst.objects().is_empty());
}
