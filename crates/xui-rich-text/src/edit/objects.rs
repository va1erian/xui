#![forbid(unsafe_code)]

//! Image interaction: resizing (with a preview that is not recorded), moving
//! the anchor and changing the wrap.

use xui_core::Dip;

use super::controller::{EditorState, Effect};
use crate::model::paragraph::{Paragraph, Span};
use crate::model::selection::DocRange;
use crate::model::{
    DocPos, EditOp, InlineImage, OBJECT_CHAR, ObjectId, ParaStyleId, Selection, Slice, Wrap,
};

impl EditorState {
    /// Shows `id` at `size` without recording an undo step, for the frames of
    /// a drag. Call [`EditorState::cancel_preview`] to abort, or send
    /// `Command::ResizeObject` with the final size to commit (it restores the
    /// original first, so undo returns to the size before the drag).
    pub fn preview_object_size(&mut self, id: ObjectId, size: (Dip, Dip)) -> Effect {
        if self.preview.is_some_and(|(previewed, _)| previewed != id) {
            self.cancel_preview();
        }
        let Some(object) = self.doc.objects.get_mut(id) else {
            return Effect::NONE;
        };
        if self.preview.is_none() {
            self.preview = Some((id, object.size));
        }
        object.size = size;
        self.object_dirty(id)
    }

    /// Puts a previewed image back to its real size.
    pub fn cancel_preview(&mut self) -> Effect {
        let Some((id, size)) = self.preview.take() else {
            return Effect::NONE;
        };
        if let Some(object) = self.doc.objects.get_mut(id) {
            object.size = size;
        }
        self.object_dirty(id)
    }

    fn object_dirty(&self, id: ObjectId) -> Effect {
        let para = self.doc.object_pos(id).map_or(0, |p| p.para);
        Effect {
            dirty: Some(para..self.doc.paragraph_count()),
            ..Effect::NONE
        }
    }

    /// Replaces an image's value as one undo step.
    fn change_object(&mut self, id: ObjectId, change: impl FnOnce(&mut InlineImage)) -> Effect {
        self.cancel_preview();
        let Some(mut object) = self.doc.objects().get(id).cloned() else {
            return Effect::NONE;
        };
        let (size, wrap) = (object.size, object.wrap);
        change(&mut object);
        if object.size == size && object.wrap == wrap {
            return Effect::NONE;
        }
        self.run(|cx| {
            cx.apply(EditOp::SetObject { id, object })?;
            Ok(None)
        })
    }

    /// Commits a resize.
    pub(super) fn resize_object(&mut self, id: ObjectId, size: (Dip, Dip)) -> Effect {
        if self.doc.objects().get(id).is_none() {
            return Effect::NONE;
        }
        let previewed = self.preview.is_some();
        let mut effect = self.change_object(id, |o| o.size = size);
        // The preview already laid the image out, so the view must refresh
        // even when the final size equals the real one.
        if previewed && effect.dirty.is_none() {
            effect.dirty = self.object_dirty(id).dirty;
        }
        effect
    }

    /// Changes how text flows around an image.
    pub(super) fn set_wrap(&mut self, id: ObjectId, wrap: Wrap) -> Effect {
        self.change_object(id, |o| o.wrap = wrap)
    }

    /// Moves an image's anchor to `to` (a position in the document before the
    /// move) as one undo step, keeping its id, and selects it.
    pub(super) fn move_object(&mut self, id: ObjectId, to: DocPos) -> Effect {
        self.cancel_preview();
        let (Some(from), Some(object)) = (self.doc.object_pos(id), self.doc.objects().get(id))
        else {
            return Effect::NONE;
        };
        let object = object.clone();
        let len = OBJECT_CHAR.len_utf8();
        let mut to = self.snap(to);
        if to.para == from.para {
            if (from.byte..=from.byte + len).contains(&to.byte) {
                return Effect::NONE;
            }
            if to.byte > from.byte {
                to.byte -= len;
            }
        }
        let style = self.doc.paragraphs()[from.para].style_at(from.byte);
        let piece = Paragraph {
            text: OBJECT_CHAR.to_string(),
            spans: vec![Span { len, style }],
            anchors: vec![id],
            style: ParaStyleId::DEFAULT,
        };
        let range = DocRange {
            start: from,
            end: DocPos::new(from.para, from.byte + len),
        };
        self.run(|cx| {
            cx.apply(EditOp::Delete { range })?;
            let content = Slice {
                paras: vec![piece],
                objects: vec![(id, object)],
            };
            cx.apply(EditOp::Reinsert { at: to, content })?;
            Ok(Some(Selection::Object(id)))
        })
    }
}
