#![forbid(unsafe_code)]

//! Mouse presses, moves and releases.

use std::time::{Duration, Instant};

use xui_core::backend::Cursor;
use xui_core::geometry::Point;
use xui_core::message::Modifiers;

use super::drag::{Drag, Unit};
use super::events::Out;
use super::state::{Click, State};
use crate::edit::{Command, Handles};
use crate::model::{DocPos, Selection};

/// The longest gap between clicks of a multi-click.
const MULTI_CLICK: Duration = Duration::from_millis(500);
/// How far apart, in design units, clicks of a multi-click may be.
const MULTI_SLOP: i32 = 4;

impl State {
    /// The link under the position, if the character there or before it is
    /// one.
    fn link_at(&self, pos: DocPos) -> Option<String> {
        let para = self.ed.doc.paragraphs().get(pos.para)?;
        let style_at = |byte: usize| self.ed.doc.styles().char(para.style_at(byte)).link.clone();
        if pos.byte < para.text().len() {
            style_at(pos.byte).or_else(|| pos.byte.checked_sub(1).and_then(style_at))
        } else {
            style_at(pos.byte.saturating_sub(1))
        }
    }

    /// The resize handle of the selected image under the client point.
    fn handle_at(&self, point: Point) -> Option<crate::edit::Handle> {
        let (_, rect) = self.image?;
        Handles::new(self.to_view(rect), self.dpi).handle_at(point)
    }

    /// Counts a click: 2 for a double click event, 3 for a press that follows
    /// one closely, else 1.
    fn count_click(&mut self, point: Point, double: bool) -> u8 {
        let now = Instant::now();
        let slop = MULTI_SLOP * self.dpi as i32 / 96;
        let follows = self.last_click.is_some_and(|c| {
            now.duration_since(c.at) < MULTI_CLICK
                && (c.point.x - point.x).abs() <= slop
                && (c.point.y - point.y).abs() <= slop
        });
        let previous = self.last_click.map_or(0, |c| c.count);
        let count = match (double, follows && previous == 2) {
            (true, _) => 2,
            (false, true) => 3,
            (false, false) => 1,
        };
        self.last_click = Some(Click {
            at: now,
            point,
            count,
        });
        count
    }

    /// A left press (or double click) at the client point.
    pub fn press(&mut self, point: Point, mods: Modifiers, double: bool, out: &mut Out) {
        self.ready();
        out.focus = true;
        out.invalidate = true;
        if self.on_bar(point.x, point.y) {
            out.capture = self.press_bar(point.y).then_some(true);
            return;
        }
        self.pointer = point;
        if let Some((id, _)) = self.image
            && let Some(handle) = self.handle_at(point)
        {
            self.begin_resize(id, handle, point);
            out.capture = Some(true);
            return;
        }
        let at = self.to_layout(point);
        if let Some(id) = self.layout.object_at(at) {
            if self.ed.selection != Selection::Object(id) {
                out.absorb(&self.run(Command::SelectObject(id)));
            }
            self.drag = Some(Drag::Move {
                id,
                origin: point,
                moving: false,
                drop: None,
            });
            out.capture = Some(true);
            return;
        }
        let pos = self.layout.pos_at(&self.ed.doc, at);
        if mods.ctrl
            && let Some(url) = self.link_at(pos)
        {
            out.link = Some(url);
            return;
        }
        let count = self.count_click(point, double);
        let command = match count {
            1 => Command::SetCaret {
                pos,
                extend: mods.shift,
            },
            2 => Command::SelectWord(pos),
            _ => Command::SelectParagraph(pos),
        };
        out.absorb(&self.run(command));
        let origin = self.ed.doc.selection_range(&self.ed.selection);
        let unit = match origin {
            Some(origin) if count > 1 => Unit::Run {
                origin,
                paragraph: count > 2,
            },
            _ => Unit::Char,
        };
        self.drag = Some(Drag::Select(unit));
        out.capture = Some(true);
    }

    /// A pointer move to the client point.
    pub fn pointer_moved(&mut self, point: Point, mods: Modifiers, out: &mut Out) {
        self.ready();
        if self.drag_bar(point.y) {
            out.invalidate = true;
            return;
        }
        if self.drag.is_some() {
            if self.drag_to(point, mods.shift) {
                out.selection = true;
                out.invalidate = true;
            }
            return;
        }
        self.pointer = point;
        let cursor = self.cursor_at(point, mods);
        if cursor != self.cursor {
            self.cursor = cursor;
            out.cursor = Some(cursor);
        }
    }

    /// The cursor to show at the client point.
    fn cursor_at(&self, point: Point, mods: Modifiers) -> Cursor {
        if self.on_bar(point.x, point.y) {
            return Cursor::Default;
        }
        if let Some(handle) = self.handle_at(point) {
            return handle.cursor();
        }
        let at = self.to_layout(point);
        if self.layout.object_at(at).is_some() {
            return Cursor::Default;
        }
        let over_link = mods.ctrl && self.link_at(self.layout.pos_at(&self.ed.doc, at)).is_some();
        if over_link {
            Cursor::Hand
        } else {
            Cursor::Text
        }
    }

    /// A left release.
    pub fn released(&mut self, out: &mut Out) {
        if self.release_bar() {
            out.capture = Some(false);
            out.invalidate = true;
            return;
        }
        if self.drag.is_some() {
            self.end_drag(out);
            out.capture = Some(false);
        }
    }

    /// The mouse capture was taken away: abandon any drag.
    pub fn capture_lost(&mut self, out: &mut Out) {
        self.release_bar();
        if self.cancel_drag() {
            out.invalidate = true;
        }
    }
}
