#![forbid(unsafe_code)]

//! Mouse drags: extending a selection, resizing a selected image by a handle
//! and moving an image to a drop point.

use xui_core::geometry::Point;
use xui_core::{Dip, Px};

use super::events::Out;
use super::state::State;
use crate::edit::{Command, Handle, resize};
use crate::model::{DocPos, DocRange, ObjectId};

/// How far the pointer must travel, in design units, before a press on an
/// image counts as a drag.
const SLOP: Dip = Dip(4.0);

/// What a selection drag extends by.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Unit {
    /// Character by character from the press.
    Char,
    /// Whole words (or paragraphs) from a double (or triple) click, keeping
    /// the one first selected in the selection.
    Run {
        /// The word or paragraph the click selected.
        origin: DocRange,
        /// Whether the unit is the paragraph rather than the word.
        paragraph: bool,
    },
}

/// A drag in progress.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Drag {
    /// Extending the text selection.
    Select(Unit),
    /// Resizing an image by one of its handles.
    Resize {
        id: ObjectId,
        handle: Handle,
        origin: Point,
        start: (Dip, Dip),
        /// The size last previewed.
        last: Option<(Dip, Dip)>,
    },
    /// Moving an image by its body.
    Move {
        id: ObjectId,
        origin: Point,
        moving: bool,
        drop: Option<DocPos>,
    },
}

impl State {
    /// Starts resizing image `id` by `handle` from the client point `origin`.
    pub fn begin_resize(&mut self, id: ObjectId, handle: Handle, origin: Point) {
        let start = self
            .ed
            .doc
            .objects()
            .get(id)
            .map_or((Dip(0.0), Dip(0.0)), |o| o.size);
        self.drag = Some(Drag::Resize {
            id,
            handle,
            origin,
            start,
            last: None,
        });
    }

    /// Extends the selection to the pointer by `unit`. By word or paragraph
    /// the one first selected stays selected, whichever way the drag goes.
    fn extend_selection(&mut self, unit: Unit, to: Point) {
        let pos = self.pos_at_view(to);
        let Unit::Run { origin, paragraph } = unit else {
            self.run(Command::SetCaret { pos, extend: true });
            return;
        };
        self.run(if paragraph {
            Command::SelectParagraph(pos)
        } else {
            Command::SelectWord(pos)
        });
        let Some(hit) = self.ed.doc.selection_range(&self.ed.selection) else {
            return;
        };
        let (anchor, head) = if hit.start < origin.start {
            (origin.end, hit.start)
        } else {
            (origin.start, hit.end.max(origin.end))
        };
        self.run(Command::SetCaret {
            pos: anchor,
            extend: false,
        });
        self.run(Command::SetCaret {
            pos: head,
            extend: true,
        });
    }

    /// Follows the pointer to the client point `to` during a drag.
    pub fn drag_to(&mut self, to: Point, free_aspect: bool) -> bool {
        self.pointer = to;
        match self.drag {
            Some(Drag::Select(unit)) => {
                self.extend_selection(unit, to);
                true
            }
            Some(Drag::Resize {
                id,
                handle,
                origin,
                start,
                ..
            }) => {
                let delta = (to.x - origin.x, to.y - origin.y);
                let max = Px(self.layout.width() as i32).to_dip(self.dpi);
                let size = resize(handle, start, delta, self.dpi, !free_aspect, max);
                self.preview_size(id, size);
                self.drag = Some(Drag::Resize {
                    id,
                    handle,
                    origin,
                    start,
                    last: Some(size),
                });
                true
            }
            Some(Drag::Move {
                id, origin, moving, ..
            }) => {
                let slop = SLOP.to_px(self.dpi).value();
                let far = (to.x - origin.x).abs().max((to.y - origin.y).abs()) > slop;
                if !moving && !far {
                    return false;
                }
                let drop = self.pos_at_view(to);
                self.drop_caret = Some(self.layout.caret_rect(&self.ed.doc, drop));
                self.drag = Some(Drag::Move {
                    id,
                    origin,
                    moving: true,
                    drop: Some(drop),
                });
                true
            }
            None => false,
        }
    }

    /// Ends the drag at the pointer's release, committing it.
    pub fn end_drag(&mut self, out: &mut Out) {
        self.drop_caret = None;
        match self.drag.take() {
            Some(Drag::Resize {
                id,
                last: Some(size),
                ..
            }) => out.absorb(&self.run(Command::ResizeObject { id, size })),
            Some(Drag::Resize { .. }) => self.cancel_preview(),
            Some(Drag::Move {
                id,
                moving: true,
                drop: Some(to),
                ..
            }) => out.absorb(&self.run(Command::MoveObject { id, to })),
            _ => {}
        }
        out.invalidate = true;
    }

    /// Abandons the drag without committing it (Escape, or losing the capture).
    pub fn cancel_drag(&mut self) -> bool {
        self.drop_caret = None;
        match self.drag.take() {
            Some(Drag::Resize { .. }) => {
                self.cancel_preview();
                true
            }
            Some(_) => true,
            None => false,
        }
    }

    /// One tick of autoscroll: while a selection or image drag is held outside
    /// the viewport, scrolls towards the pointer and follows it.
    pub fn autoscroll(&mut self) -> bool {
        let active = matches!(
            self.drag,
            Some(Drag::Select(_) | Drag::Move { moving: true, .. })
        );
        let (y, height) = (self.pointer.y, self.viewport as i32);
        if !active || (0..height).contains(&y) {
            return false;
        }
        let beyond = if y < 0 { y } else { y - height };
        let before = self.scroll;
        self.scroll_by(beyond as f32 / 2.0);
        if self.scroll == before {
            return false;
        }
        self.drag_to(self.pointer, false);
        true
    }
}
