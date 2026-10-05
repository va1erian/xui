#![forbid(unsafe_code)]

//! NetSurf's pointer shapes as xui cursors.
//!
//! NetSurf tells the host which pointer to show (`gui_pointer_shape`, in
//! `netsurf/mouse.h`) as the pointer moves over links, text fields and the
//! page's own `cursor:` styles. The numbers below mirror that header's enum, in
//! declaration order.

use xui_core::backend::Cursor;

const POINT: i32 = 1;
const CARET: i32 = 2;
const UP: i32 = 4;
const DOWN: i32 = 5;
const LEFT: i32 = 6;
const RIGHT: i32 = 7;
const RU: i32 = 8;
const LD: i32 = 9;
const LU: i32 = 10;
const RD: i32 = 11;
const WAIT: i32 = 14;
const PROGRESS: i32 = 18;

/// The xui cursor for a `gui_pointer_shape`. Shapes with no counterpart (the
/// menu, cross and move pointers, drag and drop) are the plain arrow.
pub(crate) fn cursor_for(shape: i32) -> Cursor {
    match shape {
        POINT => Cursor::Hand,
        CARET => Cursor::Text,
        WAIT | PROGRESS => Cursor::Busy,
        LEFT | RIGHT => Cursor::SizeHorizontal,
        UP | DOWN => Cursor::SizeVertical,
        LU | RD => Cursor::SizeNwSe,
        RU | LD => Cursor::SizeNeSw,
        _ => Cursor::Default,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn links_text_and_busy_map_to_their_cursors() {
        assert_eq!(cursor_for(POINT), Cursor::Hand);
        assert_eq!(cursor_for(CARET), Cursor::Text);
        assert_eq!(cursor_for(WAIT), Cursor::Busy);
        assert_eq!(cursor_for(PROGRESS), Cursor::Busy);
    }

    #[test]
    fn resize_pointers_follow_their_axis() {
        assert_eq!(cursor_for(LEFT), Cursor::SizeHorizontal);
        assert_eq!(cursor_for(RIGHT), Cursor::SizeHorizontal);
        assert_eq!(cursor_for(UP), Cursor::SizeVertical);
        assert_eq!(cursor_for(DOWN), Cursor::SizeVertical);
        assert_eq!(cursor_for(LU), Cursor::SizeNwSe);
        assert_eq!(cursor_for(RD), Cursor::SizeNwSe);
        assert_eq!(cursor_for(RU), Cursor::SizeNeSw);
        assert_eq!(cursor_for(LD), Cursor::SizeNeSw);
    }

    #[test]
    fn everything_else_is_the_arrow() {
        // DEFAULT, MENU, CROSS, MOVE, HELP, NO_DROP, NOT_ALLOWED, and values
        // a newer NetSurf might add.
        for shape in [0, 3, 12, 13, 15, 16, 17, 19, -1, i32::MAX] {
            assert_eq!(cursor_for(shape), Cursor::Default, "{shape}");
        }
    }
}
