#![forbid(unsafe_code)]

//! What is drawn over the text: the caret, the drop caret of an image drag,
//! and the outline and handles of the selected image.

use xui_core::Theme;
use xui_core::backend::Canvas;
use xui_core::geometry::Rect;

use super::state::State;
use crate::edit::{Handle, Handles};
use crate::model::Selection;

/// The caret's width in device pixels at 96 dpi.
const CARET_WIDTH: i32 = 1;

/// Paints the overlays in view coordinates: `pad` is the text margin and
/// `shift` the scroll offset.
pub(crate) fn paint(canvas: &mut dyn Canvas, state: &State, theme: &Theme, pad: i32, shift: i32) {
    let view = |r: Rect| r.offset(pad, -shift);
    let width = (CARET_WIDTH * state.dpi as i32 / 96).max(1);
    let caret = |canvas: &mut dyn Canvas, r: Rect, color| {
        let r = view(r);
        canvas.fill_rect(Rect::new(r.left, r.top, r.left + width, r.bottom), color);
    };
    if let (Selection::Text { head, anchor }, true, true) =
        (state.ed.selection, state.focused, state.caret_on)
        && head == anchor
    {
        let at = state.layout.caret_rect(&state.ed.doc, head);
        caret(canvas, at, theme.text);
    }
    if let Some(drop) = state.drop_caret {
        caret(canvas, drop, theme.accent);
    }
    if let Some((_, rect)) = state.image {
        let rect = view(rect);
        canvas.stroke_rect(rect, theme.accent, 2.0);
        let handles = Handles::new(rect, state.dpi);
        for handle in Handle::ALL {
            let square = handles.rect(handle);
            canvas.fill_rect(square, theme.input_background);
            canvas.stroke_rect(square, theme.accent, 1.0);
        }
    }
}
