#![forbid(unsafe_code)]

//! Style and object edits, and the exact restores their inverses use.

use std::ops::Range;

use super::{EditError, EditOp};
use crate::model::paragraph::Span;
use crate::model::patch::{CharStylePatch, ParaStylePatch};
use crate::model::pieces::{map_range, normalize};
use crate::model::selection::DocRange;
use crate::model::style::{CharStyleId, ParaStyleId};
use crate::model::{Document, InlineImage, ObjectId};

impl Document {
    pub(super) fn op_set_char_style(
        &mut self,
        range: DocRange,
        patch: &CharStylePatch,
    ) -> Result<EditOp, EditError> {
        self.check_range(range)?;
        let mut cache: Vec<(CharStyleId, CharStyleId)> = Vec::new();
        let mut undo = Vec::new();
        for (index, lo, hi) in self.covered(range) {
            let styles = &mut self.styles;
            let para = &mut self.paragraphs[index];
            let mut restyle = |id: CharStyleId| {
                if let Some(&(_, done)) = cache.iter().find(|&&(from, _)| from == id) {
                    return done;
                }
                let done = styles.intern_char(patch.apply(styles.char(id)));
                cache.push((id, done));
                done
            };
            let spans = map_range(&para.spans, lo, hi, &mut restyle);
            let fallback = if para.text.is_empty() {
                restyle(para.spans[0].style)
            } else {
                para.spans[0].style
            };
            let spans = normalize(spans, fallback);
            if spans != para.spans {
                undo.push((index, std::mem::replace(&mut para.spans, spans)));
            }
        }
        Ok(EditOp::RestoreSpans(undo))
    }

    pub(super) fn op_set_para_style(
        &mut self,
        paras: Range<usize>,
        patch: &ParaStylePatch,
    ) -> Result<EditOp, EditError> {
        if paras.start > paras.end || paras.end > self.paragraphs.len() {
            return Err(EditError::BadParagraphs(paras));
        }
        let mut undo = Vec::new();
        for index in paras {
            let old = self.paragraphs[index].style;
            let new = self.styles.intern_para(patch.apply(self.styles.para(old)));
            if new != old {
                self.paragraphs[index].style = new;
                undo.push((index, old));
            }
        }
        Ok(EditOp::RestoreParaStyles(undo))
    }

    pub(super) fn op_set_object(
        &mut self,
        id: ObjectId,
        object: InlineImage,
    ) -> Result<EditOp, EditError> {
        let slot = self
            .objects
            .get_mut(id)
            .ok_or(EditError::UnknownObject(id))?;
        let old = std::mem::replace(slot, object);
        Ok(EditOp::SetObject { id, object: old })
    }

    pub(super) fn op_restore_spans(
        &mut self,
        entries: Vec<(usize, Vec<Span>)>,
    ) -> Result<EditOp, EditError> {
        if let Some(&(index, _)) = entries.iter().find(|(i, _)| *i >= self.paragraphs.len()) {
            return Err(EditError::BadParagraphs(index..index + 1));
        }
        // Inverse ops are public values, so a caller can build spans that do
        // not fit the text or name styles of another document: check them all
        // before changing anything.
        for (index, spans) in &entries {
            for span in spans {
                self.check_char_style(span.style)?;
            }
            let para = &mut self.paragraphs[*index];
            let old = std::mem::replace(&mut para.spans, spans.clone());
            let fits = para.check().is_ok();
            para.spans = old;
            if !fits {
                return Err(EditError::BadParagraphs(*index..*index + 1));
            }
        }
        let undo = entries
            .into_iter()
            .map(|(index, spans)| {
                (
                    index,
                    std::mem::replace(&mut self.paragraphs[index].spans, spans),
                )
            })
            .collect();
        Ok(EditOp::RestoreSpans(undo))
    }

    pub(super) fn op_restore_para_styles(
        &mut self,
        entries: Vec<(usize, ParaStyleId)>,
    ) -> Result<EditOp, EditError> {
        if let Some(&(index, _)) = entries.iter().find(|(i, _)| *i >= self.paragraphs.len()) {
            return Err(EditError::BadParagraphs(index..index + 1));
        }
        if entries
            .iter()
            .any(|(_, style)| style.0 as usize >= self.styles.paras().len())
        {
            return Err(EditError::UnknownStyle);
        }
        let undo = entries
            .into_iter()
            .map(|(index, style)| {
                (
                    index,
                    std::mem::replace(&mut self.paragraphs[index].style, style),
                )
            })
            .collect();
        Ok(EditOp::RestoreParaStyles(undo))
    }
}
