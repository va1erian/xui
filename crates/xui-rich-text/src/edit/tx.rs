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
        EditOp::InsertText { at, .. }
        | EditOp::SplitParagraph { at }
        | EditOp::InsertObject { at, .. }
        | EditOp::Reinsert { at, .. } => (at.para, at.para + 1),
        EditOp::Delete { range } | EditOp::Remove { range, .. } => {
            (range.start.para, range.start.para)
        }
        EditOp::SetCharStyle { range, .. } => (range.start.para, range.end.para),
        EditOp::MergeParagraph { para } => (*para, *para),
        EditOp::SetParaStyle { paras, .. } => (paras.start, paras.end.saturating_sub(1)),
        EditOp::SetObject { id, .. } => {
            let para = doc.object_pos(*id).map_or(0, |p| p.para);
            (para, para)
        }
        EditOp::RestoreSpans(_) | EditOp::RestoreParaStyles(_) => (0, doc.paragraph_count()),
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
        self.dirty.note(at.para, at.para + 1);
        self.tx.insert_fragment(at, fragment)
    }
}
