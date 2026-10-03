#![forbid(unsafe_code)]

//! Text edits: the remove / reinsert primitives and the ops built on them.

use super::{EditError, EditOp};
use crate::model::object::OBJECT_CHAR;
use crate::model::paragraph::Paragraph;
use crate::model::selection::DocRange;
use crate::model::style::{CharStyleId, ParaStyle, ParaStyleId};
use crate::model::table::{Table, TableId};
use crate::model::{DocPos, Document, InlineImage, ObjectId};

/// Content cut out of a document, exact enough to put back: whole paragraphs
/// (the first and last are the partial pieces at the ends of the range, in the
/// document's own style ids), the objects they anchor and the tables wholly
/// inside them.
#[derive(Clone, Debug)]
pub struct Slice {
    pub(crate) paras: Vec<Paragraph>,
    pub(crate) objects: Vec<(ObjectId, InlineImage)>,
    pub(crate) tables: Vec<(TableId, Table)>,
}

impl Slice {
    /// Plain paragraphs with no objects or tables.
    pub(crate) fn paras(paras: Vec<Paragraph>) -> Slice {
        Slice {
            paras,
            objects: Vec::new(),
            tables: Vec::new(),
        }
    }
}

/// The text with the characters a paragraph cannot hold removed.
pub(crate) fn sanitize(text: &str) -> String {
    text.chars()
        .filter(|&c| c != OBJECT_CHAR && c != '\r')
        .collect()
}

/// Where the caret ends up after `text` (already sanitized) is inserted at `at`.
pub(crate) fn end_after(at: DocPos, text: &str) -> DocPos {
    match text.rsplit_once('\n') {
        None => DocPos::new(at.para, at.byte + text.len()),
        Some((before, last)) => DocPos::new(at.para + before.matches('\n').count() + 1, last.len()),
    }
}

impl Document {
    /// A copy of the content of `range`, which must be valid.
    pub(crate) fn slice_range(&self, range: DocRange) -> Slice {
        let mut paras = Vec::new();
        let mut objects = Vec::new();
        for p in range.start.para..=range.end.para {
            let para = &self.paragraphs[p];
            let lo = if p == range.start.para {
                range.start.byte
            } else {
                0
            };
            let hi = if p == range.end.para {
                range.end.byte
            } else {
                para.text.len()
            };
            let piece = para.slice(lo, hi);
            for &id in &piece.anchors {
                if let Some(object) = self.objects.get(id) {
                    objects.push((id, object.clone()));
                }
            }
            paras.push(piece);
        }
        Slice {
            paras,
            objects,
            tables: Vec::new(),
        }
    }

    /// Removes `range` (valid, its ends in one cell or both outside tables),
    /// returning the op that puts it back. Tables wholly inside the range go
    /// with it.
    pub(crate) fn remove(&mut self, range: DocRange, keep: CharStyleId) -> EditOp {
        let mut content = self.slice_range(range);
        for (id, _) in &content.objects {
            self.objects.remove(*id);
        }

        let first = &self.paragraphs[range.start.para];
        let last = &self.paragraphs[range.end.para];
        let head = first.slice(0, range.start.byte);
        let tail = last.slice(range.end.byte, last.text.len());
        let merged = Paragraph::concat(&[&head, &tail], keep, first.style);
        self.paragraphs
            .splice(range.start.para..=range.end.para, [merged]);
        content.tables = self.take_unused_tables(&content.paras);
        EditOp::Reinsert {
            at: range.start,
            content,
        }
    }

    /// Puts `content` at `at` (valid), returning the op that removes it again.
    ///
    /// The first paragraph of the content joins the paragraph at `at`, which
    /// keeps its style and cell; the last one takes the text after `at`.
    /// Inside a cell every new paragraph continues that cell (tables do not
    /// nest); outside, the content's own cells, and so its tables, are kept.
    pub(crate) fn reinsert(&mut self, at: DocPos, content: Slice) -> Result<EditOp, EditError> {
        let (Some(first), Some(last)) = (content.paras.first(), content.paras.last()) else {
            return Err(EditError::BadParagraphs(0..0));
        };
        let keep = self.typing_style(at);
        let target = &self.paragraphs[at.para];
        let head = target.slice(0, at.byte);
        let tail = target.slice(at.byte, target.text.len());
        let count = content.paras.len();
        let cell = target.cell;
        let inner = cell.map(|c| c.continued());
        let (mut replacement, end) = if count == 1 {
            let joined =
                Paragraph::concat(&[&head, first, &tail], first.spans[0].style, target.style);
            (
                vec![joined],
                DocPos::new(at.para, at.byte + first.text.len()),
            )
        } else {
            let top = Paragraph::concat(&[&head, first], first.spans[0].style, target.style);
            let bottom = Paragraph::concat(&[last, &tail], last.spans[0].style, last.style);
            let mut paras = vec![top];
            paras.extend_from_slice(&content.paras[1..count - 1]);
            paras.push(bottom);
            (paras, DocPos::new(at.para + count - 1, last.text.len()))
        };
        let last = replacement.len() - 1;
        for (i, para) in replacement.iter_mut().enumerate() {
            if i == 0 {
                para.cell = cell;
            } else if i == last || cell.is_some() {
                para.cell = inner;
            }
        }
        self.paragraphs.splice(at.para..=at.para, replacement);
        for (id, object) in content.objects {
            self.objects.insert_with_id(id, object);
        }
        if cell.is_none() {
            for (id, table) in content.tables {
                self.tables.insert_with_id(id, table);
            }
        }
        Ok(EditOp::Remove {
            range: DocRange { start: at, end },
            keep,
        })
    }

    pub(super) fn op_insert_text(
        &mut self,
        at: DocPos,
        text: &str,
        style: Option<CharStyleId>,
    ) -> Result<EditOp, EditError> {
        self.check_pos(at)?;
        let style = match style {
            Some(style) => {
                self.check_char_style(style)?;
                style
            }
            None => self.typing_style(at),
        };
        let para_style = self.paragraphs[at.para].style;
        let rest_style = self.continued_style(para_style);
        let paras = sanitize(text)
            .split('\n')
            .enumerate()
            .map(|(i, line)| {
                let para_style = if i == 0 { para_style } else { rest_style };
                Paragraph::new(line, para_style, style)
            })
            .collect();
        self.reinsert(at, Slice::paras(paras))
    }

    /// `id` without the page break before it: the style a paragraph split
    /// off (or typed after) one carries on in, so Enter does not start
    /// another page.
    fn continued_style(&mut self, id: ParaStyleId) -> ParaStyleId {
        let style = self.styles.para(id);
        if !style.page_break_before {
            return id;
        }
        let style = ParaStyle {
            page_break_before: false,
            ..style.clone()
        };
        self.styles.intern_para(style)
    }

    pub(super) fn op_split(&mut self, at: DocPos) -> Result<EditOp, EditError> {
        self.check_pos(at)?;
        let style = self.paragraphs[at.para].style;
        let typing = self.typing_style(at);
        let piece = Paragraph::new("", style, typing);
        let rest = Paragraph::new("", self.continued_style(style), typing);
        self.reinsert(at, Slice::paras(vec![piece, rest]))
    }

    pub(super) fn op_merge(&mut self, para: usize) -> Result<EditOp, EditError> {
        if para + 1 >= self.paragraphs.len() || self.region(para) != self.region(para + 1) {
            return Err(EditError::BadParagraphs(para..para + 2));
        }
        let len = self.paragraphs[para].text.len();
        let keep = self.paragraphs[para].typing_style(len);
        let range = DocRange {
            start: DocPos::new(para, len),
            end: DocPos::new(para + 1, 0),
        };
        Ok(self.remove(range, keep))
    }

    pub(super) fn op_insert_object(
        &mut self,
        at: DocPos,
        object: InlineImage,
    ) -> Result<EditOp, EditError> {
        self.check_pos(at)?;
        let id = self.objects.insert(object.clone());
        let style = self.paragraphs[at.para].style;
        let mut piece = Paragraph::new("", style, self.typing_style(at));
        piece.text.push(OBJECT_CHAR);
        piece.spans[0].len = OBJECT_CHAR.len_utf8();
        piece.anchors.push(id);
        self.reinsert(
            at,
            Slice {
                paras: vec![piece],
                objects: vec![(id, object)],
                tables: Vec::new(),
            },
        )
    }
}
