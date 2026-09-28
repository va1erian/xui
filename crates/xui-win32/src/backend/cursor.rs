#![forbid(unsafe_code)]

//! The pointer shape the Win32 backend shows for a portable [`Cursor`].

use xui_core::backend::Cursor;

use crate::window::CursorShape;

/// The Win32 pointer shape for a portable cursor.
pub(super) fn cursor_shape(cursor: Cursor) -> CursorShape {
    match cursor {
        Cursor::Default => CursorShape::Arrow,
        Cursor::Hand => CursorShape::Hand,
        Cursor::Text => CursorShape::IBeam,
        Cursor::SizeHorizontal => CursorShape::SizeHorizontal,
        Cursor::SizeVertical => CursorShape::SizeVertical,
        Cursor::SizeNwSe => CursorShape::SizeNwSe,
        Cursor::SizeNeSw => CursorShape::SizeNeSw,
    }
}

#[cfg(test)]
mod tests {
    use super::cursor_shape;
    use crate::window::CursorShape;
    use xui_core::backend::Cursor;

    #[test]
    fn the_resize_cursors_map_to_their_win32_shapes() {
        assert_eq!(
            cursor_shape(Cursor::SizeHorizontal),
            CursorShape::SizeHorizontal
        );
        assert_eq!(
            cursor_shape(Cursor::SizeVertical),
            CursorShape::SizeVertical
        );
        assert_eq!(cursor_shape(Cursor::SizeNwSe), CursorShape::SizeNwSe);
        assert_eq!(cursor_shape(Cursor::SizeNeSw), CursorShape::SizeNeSw);
        assert_eq!(cursor_shape(Cursor::Default), CursorShape::Arrow);
    }
}
