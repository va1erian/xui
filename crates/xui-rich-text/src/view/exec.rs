#![forbid(unsafe_code)]

//! Running commands against the editor state and bringing the layout, the
//! scroll position and the selected-image box up to date afterwards.

use std::ops::Range;

use xui_core::geometry::Rect;

use super::state::State;
use crate::edit::{Command, Effect, LineNav};
use crate::layout::Layout;
use crate::model::{DocPos, Selection};

/// Answers caret-movement questions from the layout.
pub(crate) struct Nav<'a> {
    layout: &'a Layout,
    page: i32,
}

impl LineNav for Nav<'_> {
    fn line_start(&self, pos: DocPos) -> DocPos {
        self.layout.line_start(pos)
    }

    fn line_end(&self, pos: DocPos) -> DocPos {
        self.layout.line_end(pos)
    }

    fn vertical(&self, pos: DocPos, sticky_x: Option<f32>, lines: i32) -> (DocPos, f32) {
        self.layout.vertical(pos, sticky_x, lines)
    }

    fn page_lines(&self) -> i32 {
        self.page
    }
}

impl State {
    /// Runs `command` as one undoable step and refreshes the view.
    pub fn run(&mut self, command: Command) -> Effect {
        self.ready();
        let at = self.selection_para();
        self.ensure_para(at);
        let now = self.epoch.elapsed();
        let nav = Nav {
            layout: &self.layout,
            page: self.layout.page_lines(self.viewport),
        };
        let effect = self.ed.exec(command, &nav, now, self.clipboard.as_ref());
        self.refresh(&effect);
        effect
    }

    /// Previews an image size (a resize drag's frame) without an undo step.
    pub fn preview_size(
        &mut self,
        id: crate::model::ObjectId,
        size: (xui_core::Dip, xui_core::Dip),
    ) {
        let effect = self.ed.preview_object_size(id, size);
        self.refresh_layout(&effect);
        self.sync_image();
    }

    /// Puts a previewed image back to its real size.
    pub fn cancel_preview(&mut self) {
        let effect = self.ed.cancel_preview();
        self.refresh_layout(&effect);
        self.sync_image();
    }

    /// Applies what `effect` says changed.
    fn refresh(&mut self, effect: &Effect) {
        self.refresh_layout(effect);
        self.sync_image();
        if !effect.is_none() {
            self.caret_on = true;
            self.ensure_selection_visible();
        }
    }

    fn refresh_layout(&mut self, effect: &Effect) {
        if let Some(dirty) = effect.dirty.clone() {
            self.mark_dirty(dirty);
        }
        self.ready();
    }

    /// Marks `dirty` paragraphs for relayout; when the paragraph count changed
    /// everything from the first one is laid out again.
    fn mark_dirty(&mut self, dirty: Range<usize>) {
        let old = self.layout.paragraphs().len();
        let new = self.ed.doc.paragraph_count();
        if old == new {
            dirty.for_each(|i| self.layout.mark_dirty(i));
        } else {
            let start = dirty.start.min(old);
            self.layout.splice(start, old - start, new - start);
        }
    }

    /// Recomputes where the selected image is drawn.
    pub fn sync_image(&mut self) {
        let at = self.selection_para();
        self.ensure_para(at);
        self.image = match self.ed.selection {
            Selection::Object(id) => self
                .ed
                .doc
                .object_pos(id)
                .and_then(|at| self.layout.object_rect(at.para, id))
                .map(|rect| (id, rect)),
            Selection::Text { .. } => None,
        };
    }

    /// The paragraph holding the caret, or the selected image's anchor.
    fn selection_para(&self) -> usize {
        match self.ed.selection {
            Selection::Text { head, .. } => head.para,
            Selection::Object(id) => self.ed.doc.object_pos(id).map_or(0, |at| at.para),
        }
    }

    /// Scrolls the least that shows the caret, or the selected image.
    pub fn ensure_selection_visible(&mut self) {
        let at = self.selection_para();
        self.ensure_para(at);
        let rect: Option<Rect> = match self.ed.selection {
            Selection::Text { head, .. } => Some(self.layout.caret_rect(&self.ed.doc, head)),
            Selection::Object(_) => self.image.map(|(_, rect)| rect),
        };
        if let Some(rect) = rect {
            self.ensure_visible(rect);
        }
    }
}
