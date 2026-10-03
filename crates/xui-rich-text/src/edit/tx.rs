#![forbid(unsafe_code)]

//! A transaction that also tracks which paragraphs it touched.

use crate::model::{DocPos, Document, EditError, EditOp, Fragment, Transaction};

/// The paragraphs (in the coordinates they had when touched) an edit changed.
#[derive(Clone, Copy, Debug)]
pub(super) struct Dirty {
    pub first: usize,
    pub last: usize,
}

impl Default for Dirty {
    fn default() -> Dirty {
        Dirty {
            first: usize::MAX,
            last: 0,
        }
    }
}

impl Dirty {
    pub fn note(&mut self, first: usize, last: usize) {
        self.first = self.first.min(first);
        self.last = self.last.max(last);
    }

    pub fn is_clean(&self) -> bool {
        self.first == usize::MAX
    }
}

/// The paragraphs `op` changes.
fn span(doc: &Document, op: &EditOp) -> (usize, usize) {
    match op {
        // Inserted text or content can span several paragraphs; all of them
        // change even when an earlier delete keeps the paragraph count equal.
        EditOp::InsertText { at, text, .. } => {
            (at.para, at.para + text.matches('\n').count().max(1))
        }
        EditOp::Reinsert { at, content } => (at.para, at.para + content.paras.len().max(1)),
        EditOp::SplitParagraph { at } | EditOp::InsertObject { at, .. } => (at.para, at.para + 1),
        // A delete across cells clears each cell it covers.
        EditOp::Delete { range } | EditOp::Remove { range, .. } => {
            (range.start.para, range.end.para)
        }
        EditOp::SetCharStyle { range, .. } => (range.start.para, range.end.para),
        EditOp::MergeParagraph { para } => (*para, *para),
        EditOp::SetParaStyle { paras, .. } => (paras.start, paras.end.saturating_sub(1)),
        EditOp::SetObject { id, .. } => {
            let para = doc.object_pos(*id).map_or(0, |p| p.para);
            (para, para)
        }
        EditOp::RestoreSpans(_) | EditOp::RestoreParaStyles(_) => (0, doc.paragraph_count()),
        // The view relays out every paragraph when the page changes.
        EditOp::SetPage(_) => (0, 0),
        EditOp::InsertParas { at, content } => (*at, at + content.paras.len()),
        EditOp::RemoveParas { paras } => (paras.start, paras.start),
        EditOp::SetTable { id, .. } => doc
            .table_spans()
            .into_iter()
            .find(|t| t.id == *id)
            .map_or((0, 0), |t| (t.paras.start, t.paras.end)),
        EditOp::SetCells(entries) => entries
            .iter()
            .fold((usize::MAX, 0), |(lo, hi), (i, _)| (lo.min(*i), hi.max(*i))),
        EditOp::Batch(_) => (0, doc.paragraph_count()),
    }
}

/// The handle a command body edits through.
pub(super) struct Cx<'a, 'b> {
    tx: &'a mut Transaction<'b>,
    dirty: &'a mut Dirty,
}

impl<'a, 'b> Cx<'a, 'b> {
    pub fn new(tx: &'a mut Transaction<'b>, dirty: &'a mut Dirty) -> Cx<'a, 'b> {
        Cx { tx, dirty }
    }

    /// The document as edited so far.
    pub fn doc(&self) -> &Document {
        self.tx.doc()
    }

    /// Applies `op`, noting the paragraphs it touches.
    pub fn apply(&mut self, op: EditOp) -> Result<(), EditError> {
        let (first, last) = span(self.tx.doc(), &op);
        self.dirty.note(first, last);
        self.tx.apply(op)
    }

    /// Inserts a fragment, returning the position after it.
    pub fn insert_fragment(
        &mut self,
        at: DocPos,
        fragment: &Fragment,
    ) -> Result<DocPos, EditError> {
        let paras = fragment.paragraphs().len().max(1);
        self.dirty.note(at.para, at.para + paras);
        self.tx.insert_fragment(at, fragment)
    }
}
