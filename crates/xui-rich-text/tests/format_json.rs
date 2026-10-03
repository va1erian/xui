//! The JSON save format: round trips and rejection of invalid files.
#![cfg(feature = "serde")]

#[path = "model_common.rs"]
mod common;

use common::{apply, bold, object, range, resolve, spec, start_doc, text};
use proptest::prelude::*;
use serde_json::{Value, json};
use xui_core::{Color, Dip};
use xui_rich_text::format::{FormatError, from_json, to_json};
use xui_rich_text::model::{
    Align, BlockKind, CharStylePatch, DocPos, Document, EditOp, ListItem, ListKind, ParaStylePatch,
    TextColor,
};

/// A document that uses most of the model.
fn rich_doc() -> Document {
    let mut doc = Document::from_plain_text("Title\nsome bold and linked text\n\nlast");
    bold(&mut doc, range(1, 5, 1, 9));
    apply(
        &mut doc,
        EditOp::SetCharStyle {
            range: range(1, 14, 1, 20),
            patch: CharStylePatch {
                link: Some(Some("https://example.com/a b".into())),
                color: Some(TextColor::Fixed(Color::rgb(1, 2, 3))),
                highlight: Some(Some(Color::rgb(250, 250, 0))),
                size: Some(Dip(18.5)),
                family: Some(Some("Georgia".into())),
                ..CharStylePatch::italic(true)
            },
        },
    );
    apply(
        &mut doc,
        EditOp::SetParaStyle {
            paras: 0..1,
            patch: ParaStylePatch {
                align: Some(Align::Center),
                kind: Some(BlockKind::Heading(1)),
                ..ParaStylePatch::default()
            },
        },
    );
    apply(
        &mut doc,
        EditOp::SetParaStyle {
            paras: 3..4,
            patch: ParaStylePatch::list(Some(ListItem {
                kind: ListKind::Numbered,
                level: 1,
            })),
        },
    );
    for (para, seed) in [(1, 3), (1, 4), (3, 5)] {
        apply(
            &mut doc,
            EditOp::InsertObject {
                at: DocPos::new(para, 2),
                object: object(seed),
            },
        );
    }
    doc
}

fn assert_same(a: &Document, b: &Document) {
    b.check().unwrap();
    assert!(a.content_eq(b));
    assert_eq!(a.paragraphs(), b.paragraphs(), "ids survive too");
}

#[test]
fn a_rich_document_round_trips() {
    let doc = rich_doc();
    let loaded = from_json(&to_json(&doc)).unwrap();
    assert_same(&doc, &loaded);
    assert_eq!(loaded.objects().len(), 3);
    assert_eq!(to_json(&loaded), to_json(&doc), "output is deterministic");
}

#[test]
fn an_empty_document_round_trips() {
    let doc = Document::new();
    assert_same(&doc, &from_json(&to_json(&doc)).unwrap());
}

proptest! {
    #[test]
    fn random_documents_round_trip(
        initial in text(),
        specs in prop::collection::vec(spec(), 0..24),
    ) {
        let mut doc = start_doc(&initial);
        for spec in &specs {
            if let Some(op) = resolve(&doc, spec) {
                doc.apply(op).unwrap();
            }
        }
        let loaded = from_json(&to_json(&doc)).unwrap();
        prop_assert_eq!(loaded.check(), Ok(()));
        prop_assert!(doc.content_eq(&loaded));
        prop_assert_eq!(doc.paragraphs(), loaded.paragraphs());
    }
}

fn saved() -> Value {
    serde_json::from_str(&to_json(&rich_doc())).unwrap()
}

fn load(value: &Value) -> Result<Document, FormatError> {
    from_json(&value.to_string())
}

fn invalid(value: &Value) {
    assert!(
        matches!(load(value), Err(FormatError::Invalid(_))),
        "{:?}",
        load(value).err()
    );
}

#[test]
fn syntax_and_version_errors() {
    assert!(matches!(from_json("not json"), Err(FormatError::Syntax(_))));
    assert!(matches!(from_json("{}"), Err(FormatError::Syntax(_))));
    assert!(matches!(
        from_json(r#"{"version":1}"#),
        Err(FormatError::Syntax(_))
    ));
    let mut value = saved();
    value["version"] = json!(2);
    assert_eq!(load(&value).err(), Some(FormatError::Version(2)));
    value["version"] = json!("1");
    assert!(matches!(load(&value), Err(FormatError::Syntax(_))));
}

#[test]
fn bad_images_are_image_errors() {
    let mut value = saved();
    value["objects"][0]["png_base64"] = json!("!!not base64!!");
    assert!(matches!(load(&value), Err(FormatError::Image(_))));
    value["objects"][0]["png_base64"] = json!("aGVsbG8=");
    assert!(matches!(load(&value), Err(FormatError::Image(_))));
    value["objects"][0]["png_base64"] = json!("iVBORw0KGgo=");
    assert!(matches!(load(&value), Err(FormatError::Image(_))));
}

#[test]
fn structural_violations_are_invalid() {
    let mutate = |f: &dyn Fn(&mut Value)| {
        let mut value = saved();
        f(&mut value);
        invalid(&value);
    };
    mutate(&|v| v["paragraphs"][0]["spans"][0][0] = json!(2));
    mutate(&|v| v["paragraphs"][1]["spans"][0][1] = json!(99));
    mutate(&|v| v["paragraphs"][0]["style"] = json!(99));
    mutate(&|v| v["paragraphs"][1]["anchors"] = json!([]));
    mutate(&|v| v["paragraphs"][1]["anchors"][0] = json!(77));
    mutate(&|v| v["paragraphs"] = json!([]));
    mutate(&|v| v["paragraphs"][2]["spans"] = json!([]));
    mutate(&|v| v["paragraphs"][0]["text"] = json!("Title\u{fffc}"));
    mutate(&|v| v["objects"][1]["id"] = v["objects"][0]["id"].clone());
    mutate(&|v| {
        let anchor = v["paragraphs"][1]["anchors"][0].clone();
        v["paragraphs"][1]["anchors"][1] = anchor;
    });
    mutate(&|v| {
        v["paragraphs"][0]["spans"] = json!([[usize::MAX, 0], [usize::MAX, 0]]);
    });
    mutate(&|v| {
        let first = v["styles"]["chars"][0].clone();
        v["styles"]["chars"].as_array_mut().unwrap().push(first);
    });
    mutate(&|v| v["styles"]["chars"][0]["size"] = json!(-1.0));
    mutate(&|v| v["objects"][0]["width"] = json!(-4.0));
}

#[test]
fn a_span_that_splits_a_character_is_invalid() {
    let mut value = saved();
    value["paragraphs"] = json!([{
        "text": "é", "spans": [[1, 0], [1, 0]], "anchors": [], "style": 0
    }]);
    value["objects"] = json!([]);
    invalid(&value);
}
