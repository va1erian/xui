#![forbid(unsafe_code)]

//! Fragments: a piece of one document that can be inserted into another, which
//! is what copy and paste move around.

use std::collections::HashMap;

use super::ops::{EditError, EditOp, Slice};
use super::paragraph::{Paragraph, Span};
use super::selection::DocRange;
use super::style::StyleTable;
use super::table::{TableId, TableTable};
use super::{DocPos, Document, ObjectId, ObjectTable};

/// A mini-document: paragraphs with the styles, objects and tables they use,
/// in ids of the fragment's own tables, independent of the document it came
/// from. Only whole tables travel; a piece of a table is plain paragraphs.
#[derive(Clone, Debug)]
pub struct Fragment {
    paragraphs: Vec<Paragraph>,
    styles: StyleTable,
    objects: ObjectTable,
    tables: TableTable,
}

impl Fragment {
    /// A fragment of `text` in the default styles, one paragraph per line.
    pub fn from_plain_text(text: &str) -> Fragment {
        let doc = Document::from_plain_text(text);
        Fragment {
            paragraphs: doc.paragraphs,
            styles: doc.styles,
            objects: doc.objects,
            tables: doc.tables,
        }
    }

    /// The paragraphs; the first and last may be partial.
    pub fn paragraphs(&self) -> &[Paragraph] {
        &self.paragraphs
    }

    /// The styles the paragraphs refer to.
    pub fn styles(&self) -> &StyleTable {
        &self.styles
    }

    /// The objects the paragraphs anchor.
    pub fn objects(&self) -> &ObjectTable {
        &self.objects
    }

    /// Whether the fragment holds no text and no object.
    pub fn is_empty(&self) -> bool {
        self.paragraphs.len() == 1 && self.paragraphs[0].text.is_empty() && self.objects.is_empty()
    }

    /// The text, objects dropped and paragraphs joined by `\n`.
    pub fn to_plain_text(&self) -> String {
        super::edit::plain_text(&self.paragraphs)
    }
}

impl Document {
    /// Copies the content of `range` into a self-contained [`Fragment`]. The
    /// tables wholly inside the range come with it when both its ends are
    /// outside tables; otherwise every paragraph comes as a plain one.
    pub fn extract_fragment(&self, range: DocRange) -> Result<Fragment, EditError> {
        self.check_range(range)?;
        let slice = self.slice_range(range);
        let keep_tables =
            self.region(range.start.para).is_none() && self.region(range.end.para).is_none();
        let mut styles = StyleTable::new();
        let mut objects = ObjectTable::new();
        let mut tables = TableTable::new();
        let mut ids: HashMap<ObjectId, ObjectId> = HashMap::new();
        let mut table_ids: HashMap<TableId, TableId> = HashMap::new();
        for (id, object) in slice.objects {
            ids.insert(id, objects.insert(object));
        }
        let paragraphs = slice
            .paras
            .into_iter()
            .map(|mut para| {
                for span in &mut para.spans {
                    span.style = styles.intern_char(self.styles.char(span.style).clone());
                }
                para.style = styles.intern_para(self.styles.para(para.style).clone());
                for id in &mut para.anchors {
                    *id = ids[id];
                }
                para.cell = para.cell.filter(|_| keep_tables).and_then(|mut cell| {
                    let table = self.tables.get(cell.table)?.clone();
                    cell.table = *table_ids
                        .entry(cell.table)
                        .or_insert_with(|| tables.insert(table));
                    Some(cell)
                });
                para
            })
            .collect();
        Ok(Fragment {
            paragraphs,
            styles,
            objects,
            tables,
        })
    }

    /// Inserts `fragment` at `at`, adding its styles and objects (under new ids)
    /// to this document. Returns the op that undoes it and the position after
    /// the inserted content.
    pub fn insert_fragment(
        &mut self,
        at: DocPos,
        fragment: &Fragment,
    ) -> Result<(EditOp, DocPos), EditError> {
        self.check_pos(at)?;
        let mut objects = Vec::new();
        let mut tables = Vec::new();
        let mut table_ids: HashMap<TableId, TableId> = HashMap::new();
        let in_cell = self.region(at.para).is_some();
        let mut paras = Vec::with_capacity(fragment.paragraphs.len());
        for source in &fragment.paragraphs {
            let mut para = source.clone();
            para.spans = para
                .spans
                .iter()
                .map(|span| Span {
                    len: span.len,
                    style: self
                        .styles
                        .intern_char(fragment.styles.char(span.style).clone()),
                })
                .collect();
            para.style = self
                .styles
                .intern_para(fragment.styles.para(para.style).clone());
            for id in &mut para.anchors {
                if let Some(object) = fragment.objects.get(*id) {
                    let new = self.objects.insert(object.clone());
                    objects.push((new, object.clone()));
                    *id = new;
                }
            }
            // Tables do not nest, so inside a cell the copied tables are
            // dropped and `reinsert` makes every paragraph part of the cell.
            para.cell = para.cell.filter(|_| !in_cell).and_then(|mut cell| {
                let table = fragment.tables.get(cell.table)?;
                cell.table = *table_ids.entry(cell.table).or_insert_with(|| {
                    let new = self.tables.insert(table.clone());
                    tables.push((new, table.clone()));
                    new
                });
                Some(cell)
            });
            paras.push(para);
        }
        let inverse = self.reinsert(
            at,
            Slice {
                paras,
                objects,
                tables,
            },
        )?;
        let end = match &inverse {
            EditOp::Remove { range, .. } => range.end,
            _ => at,
        };
        Ok((inverse, end))
    }
}
