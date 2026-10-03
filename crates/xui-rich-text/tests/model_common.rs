//! Helpers shared by the model and format tests (included with `#[path]`).
#![allow(dead_code)]

use std::sync::Arc;

use proptest::prelude::*;
use xui_core::{Dip, Image};
use xui_rich_text::model::{
    Align, BlockKind, CharStylePatch, DocPos, DocRange, Document, EditOp, InlineImage,
    ParaStylePatch, Side, Wrap,
};

/// A small solid-colour image object.
pub fn object(seed: u8) -> InlineImage {
    let pixels: Vec<u8> = (0..4 * 3 * 2)
        .map(|i| (i as u8).wrapping_mul(seed).wrapping_add(seed))
        .collect();
    InlineImage {
        image: Arc::new(Image::from_rgba(3, 2, pixels).unwrap()),
        size: (Dip(30.0 + seed as f32), Dip(20.0)),
        wrap: if seed.is_multiple_of(2) {
            Wrap::Inline
        } else {
            Wrap::square(Side::Left)
        },
        alt: format!("image {seed}"),
    }
}

/// Applies `op`, expecting success, and returns its inverse.
pub fn apply(doc: &mut Document, op: EditOp) -> EditOp {
    doc.apply(op).expect("op applies")
}

/// Makes `range` bold.
pub fn bold(doc: &mut Document, range: DocRange) -> EditOp {
    apply(
        doc,
        EditOp::SetCharStyle {
            range,
            patch: CharStylePatch::bold(true),
        },
    )
}

/// The range from `(p0, b0)` to `(p1, b1)`.
pub fn range(p0: usize, b0: usize, p1: usize, b1: usize) -> DocRange {
    DocRange::new(DocPos::new(p0, b0), DocPos::new(p1, b1))
}

/// A paragraph's runs as `(text, bold)`.
pub fn runs(doc: &Document, para: usize) -> Vec<(String, bool)> {
    let p = &doc.paragraphs()[para];
    p.runs()
        .map(|(r, id)| {
            (
                p.text()[r].to_owned(),
                doc.styles().char(id).weight.value() >= 600,
            )
        })
        .collect()
}
pub const ALPHABET: [char; 9] = ['a', 'b', ' ', 'é', '\n', '😀', '\u{301}', '\u{200D}', 'z'];

pub fn text() -> impl Strategy<Value = String> {
    prop::collection::vec(prop::sample::select(ALPHABET.to_vec()), 0..6)
        .prop_map(|chars| chars.into_iter().collect())
}

#[derive(Clone, Debug)]
pub enum Spec {
    Insert(usize, usize, String),
    Delete(usize, usize, usize, usize),
    Split(usize, usize),
    Merge(usize),
    Style(usize, usize, usize, usize, u8),
    Para(usize, usize, u8),
    Object(usize, usize, u8),
    SetObject(usize, u8),
}

pub fn spec() -> impl Strategy<Value = Spec> {
    let n = 0..64usize;
    prop_oneof![
        3 => (n.clone(), n.clone(), text()).prop_map(|(p, c, t)| Spec::Insert(p, c, t)),
        3 => (n.clone(), n.clone(), n.clone(), n.clone()).prop_map(|(a, b, c, d)| Spec::Delete(a, b, c, d)),
        1 => (n.clone(), n.clone()).prop_map(|(p, c)| Spec::Split(p, c)),
        1 => n.clone().prop_map(Spec::Merge),
        2 => (n.clone(), n.clone(), n.clone(), n.clone(), 0..6u8).prop_map(|(a, b, c, d, k)| Spec::Style(a, b, c, d, k)),
        1 => (n.clone(), n.clone(), 0..4u8).prop_map(|(a, b, k)| Spec::Para(a, b, k)),
        1 => (n.clone(), n.clone(), 0..8u8).prop_map(|(p, c, s)| Spec::Object(p, c, s)),
        1 => (n, 0..8u8).prop_map(|(i, s)| Spec::SetObject(i, s)),
    ]
}

pub fn pos(doc: &Document, p: usize, c: usize) -> DocPos {
    let para = p % doc.paragraph_count();
    let text = doc.paragraphs()[para].text();
    let mut stops: Vec<usize> = text.char_indices().map(|(i, _)| i).collect();
    stops.push(text.len());
    DocPos::new(para, stops[c % stops.len()])
}

pub fn resolve(doc: &Document, spec: &Spec) -> Option<EditOp> {
    Some(match spec {
        Spec::Insert(p, c, t) => EditOp::InsertText {
            at: pos(doc, *p, *c),
            text: t.clone(),
            style: None,
        },
        Spec::Delete(a, b, c, d) => EditOp::Delete {
            range: DocRange::new(pos(doc, *a, *b), pos(doc, *c, *d)),
        },
        Spec::Split(p, c) => EditOp::SplitParagraph {
            at: pos(doc, *p, *c),
        },
        Spec::Merge(p) => EditOp::MergeParagraph {
            para: *p % doc.paragraph_count().checked_sub(1).filter(|&n| n > 0)?,
        },
        Spec::Style(a, b, c, d, kind) => EditOp::SetCharStyle {
            range: DocRange::new(pos(doc, *a, *b), pos(doc, *c, *d)),
            patch: match kind {
                0 => CharStylePatch::bold(true),
                1 => CharStylePatch::bold(false),
                2 => CharStylePatch::italic(true),
                3 => CharStylePatch::underline(true),
                4 => CharStylePatch::strike(true),
                _ => CharStylePatch::italic(false),
            },
        },
        Spec::Para(a, b, kind) => {
            let n = doc.paragraph_count();
            let (a, b) = (a % n, b % n);
            EditOp::SetParaStyle {
                paras: a.min(b)..a.max(b) + 1,
                patch: match kind {
                    0 => ParaStylePatch::align(Align::Center),
                    1 => ParaStylePatch::kind(BlockKind::Heading(2)),
                    2 => ParaStylePatch::align(Align::Left),
                    _ => ParaStylePatch::kind(BlockKind::Body),
                },
            }
        }
        Spec::Object(p, c, s) => EditOp::InsertObject {
            at: pos(doc, *p, *c),
            object: object(*s),
        },
        Spec::SetObject(i, s) => {
            let ids: Vec<_> = doc
                .paragraphs()
                .iter()
                .flat_map(|p| p.anchors().iter().copied())
                .collect();
            if ids.is_empty() {
                return None;
            }
            EditOp::SetObject {
                id: ids[i % ids.len()],
                object: object(*s),
            }
        }
    })
}

pub fn start_doc(text: &str) -> Document {
    Document::from_plain_text(text)
}
