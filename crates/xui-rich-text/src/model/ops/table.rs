#![forbid(unsafe_code)]

//! Table edits: whole paragraphs in and out, cell marks, table settings, and
//! deleting a range that crosses cell boundaries.

use std::ops::Range;

use super::text::Slice;
use super::{EditError, EditOp};
use crate::model::selection::DocRange;
use crate::model::table::{CellMark, Table, TableId};
use crate::model::{DocPos, Document};

impl Document {
    pub(super) fn op_insert_paras(
        &mut self,
        at: usize,
        content: Slice,
    ) -> Result<EditOp, EditError> {
        if at > self.paragraphs.len() || content.paras.is_empty() {
            return Err(EditError::BadParagraphs(at..at));
        }
        let count = content.paras.len();
        self.paragraphs.splice(at..at, content.paras);
        for (id, object) in content.objects {
            self.objects.insert_with_id(id, object);
        }
        for (id, table) in content.tables {
            self.tables.insert_with_id(id, table);
        }
        Ok(EditOp::RemoveParas {
            paras: at..at + count,
        })
    }

    pub(super) fn op_remove_paras(&mut self, paras: Range<usize>) -> Result<EditOp, EditError> {
        if paras.is_empty()
            || paras.end > self.paragraphs.len()
            || paras.len() == self.paragraphs.len()
        {
            return Err(EditError::BadParagraphs(paras));
        }
        let removed: Vec<_> = self.paragraphs.drain(paras.clone()).collect();
        let objects = removed
            .iter()
            .flat_map(|p| p.anchors.iter())
            .filter_map(|&id| self.objects.remove(id).map(|o| (id, o)))
            .collect();
        let tables = self.take_unused_tables(&removed);
        Ok(EditOp::InsertParas {
            at: paras.start,
            content: Slice {
                paras: removed,
                objects,
                tables,
            },
        })
    }

    pub(super) fn op_set_table(&mut self, id: TableId, table: Table) -> Result<EditOp, EditError> {
        table.check().map_err(|_| EditError::BadTable)?;
        let slot = self.tables.get_mut(id).ok_or(EditError::BadTable)?;
        let old = std::mem::replace(slot, table);
        Ok(EditOp::SetTable { id, table: old })
    }

    pub(super) fn op_set_cells(
        &mut self,
        entries: Vec<(usize, Option<CellMark>)>,
    ) -> Result<EditOp, EditError> {
        if let Some(&(index, _)) = entries.iter().find(|(i, _)| *i >= self.paragraphs.len()) {
            return Err(EditError::BadParagraphs(index..index + 1));
        }
        let undo = entries
            .into_iter()
            .map(|(index, cell)| {
                (
                    index,
                    std::mem::replace(&mut self.paragraphs[index].cell, cell),
                )
            })
            .collect();
        Ok(EditOp::SetCells(undo))
    }

    /// Applies `ops` in order; on an error the ones applied are undone.
    pub(super) fn op_batch(&mut self, ops: Vec<EditOp>) -> Result<EditOp, EditError> {
        let mut undo = Vec::with_capacity(ops.len());
        for op in ops {
            match self.apply(op) {
                Ok(inverse) => undo.push(inverse),
                Err(error) => {
                    for inverse in undo.into_iter().rev() {
                        let _ = self.apply(inverse);
                    }
                    return Err(error);
                }
            }
        }
        undo.reverse();
        Ok(EditOp::Batch(undo))
    }

    /// Deletes `range` (valid). Within one cell, or between two positions
    /// outside tables (taking whole tables with it), it is one removal. A
    /// range with an end inside a table clears the text it covers in each
    /// cell and outside, keeping the grid.
    pub(super) fn op_delete(&mut self, range: DocRange) -> Result<EditOp, EditError> {
        let keep = self.typing_style(range.start);
        if self.same_region(range.start, range.end) {
            return Ok(self.remove(range, keep));
        }
        let mut pieces = Vec::new();
        let mut start = range.start;
        for para in range.start.para..range.end.para {
            if self.region(para) != self.region(para + 1) {
                let end = DocPos::new(para, self.paragraphs[para].text.len());
                pieces.push(DocRange { start, end });
                start = DocPos::new(para + 1, 0);
            }
        }
        pieces.push(DocRange {
            start,
            end: range.end,
        });
        // Last first, so the earlier pieces keep their positions.
        let ops = pieces
            .into_iter()
            .rev()
            .filter(|piece| !piece.is_empty())
            .map(|range| EditOp::Remove {
                range,
                keep: self.typing_style(range.start),
            })
            .collect();
        self.op_batch(ops)
    }
}
