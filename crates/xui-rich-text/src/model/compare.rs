#![forbid(unsafe_code)]

//! Comparing documents by what they contain rather than by interned ids.

use super::{Document, InlineImage};

fn same_object(a: &InlineImage, b: &InlineImage) -> bool {
    a.image.size() == b.image.size()
        && a.image.pixels() == b.image.pixels()
        && a.size == b.size
        && a.wrap == b.wrap
        && a.alt == b.alt
}

impl Document {
    /// Whether `other` has the same content: the same text, character and
    /// paragraph styles (compared by value, so differently numbered but equal
    /// style tables match) and anchored objects (compared by pixels, size, wrap
    /// and alt text, not by id) and tables (compared by their cells and
    /// settings, not by id).
    pub fn content_eq(&self, other: &Document) -> bool {
        self.paragraphs.len() == other.paragraphs.len()
            && self
                .paragraphs
                .iter()
                .zip(&other.paragraphs)
                .all(|(a, b)| self.para_eq(a, other, b))
    }

    fn para_eq(&self, a: &super::Paragraph, other: &Document, b: &super::Paragraph) -> bool {
        let table = |doc: &Document, p: &super::Paragraph| {
            p.cell.map(|c| (c.start, doc.tables.get(c.table).cloned()))
        };
        a.text == b.text
            && table(self, a) == table(other, b)
            && self.styles.para(a.style) == other.styles.para(b.style)
            && a.spans.len() == b.spans.len()
            && a.spans.iter().zip(&b.spans).all(|(x, y)| {
                x.len == y.len && self.styles.char(x.style) == other.styles.char(y.style)
            })
            && a.anchors.len() == b.anchors.len()
            && a.anchors.iter().zip(&b.anchors).all(|(x, y)| {
                match (self.objects.get(*x), other.objects.get(*y)) {
                    (Some(p), Some(q)) => same_object(p, q),
                    _ => false,
                }
            })
    }
}
