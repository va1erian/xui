//! Helpers shared by the model and format tests (included with `#[path]`).
#![allow(dead_code)]

use std::sync::Arc;

use xui_core::{Dip, Image};
use xui_rich_text::model::{
    CharStylePatch, DocPos, DocRange, Document, EditOp, InlineImage, Side, Wrap,
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
