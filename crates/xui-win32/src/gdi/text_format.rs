#![forbid(unsafe_code)]

//! How [`Canvas::draw_text`](super::Canvas::draw_text) lays text out.

// DrawText flags, mirrored here so callers never see the `windows` crate.
const DT_CENTER: u32 = 0x0000_0001;
const DT_RIGHT: u32 = 0x0000_0002;
const DT_VCENTER: u32 = 0x0000_0004;
const DT_WORDBREAK: u32 = 0x0000_0010;
const DT_SINGLELINE: u32 = 0x0000_0020;
const DT_NOPREFIX: u32 = 0x0000_0800;
const DT_END_ELLIPSIS: u32 = 0x0000_8000;

/// How [`Canvas::draw_text`](super::Canvas::draw_text) lays text out.
#[derive(Clone, Copy, Debug, Default)]
pub struct TextFormat(u32);

impl TextFormat {
    /// Left-aligned (the default).
    pub const fn left() -> TextFormat {
        TextFormat(0)
    }

    /// Horizontally centred.
    pub const fn center(self) -> TextFormat {
        TextFormat((self.0 & !DT_RIGHT) | DT_CENTER)
    }

    /// Right-aligned.
    pub const fn right(self) -> TextFormat {
        TextFormat((self.0 & !DT_CENTER) | DT_RIGHT)
    }

    /// Vertically centred within the rectangle.
    pub const fn vcenter(self) -> TextFormat {
        TextFormat(self.0 | DT_VCENTER)
    }

    /// Keep the text on a single line.
    pub const fn single_line(self) -> TextFormat {
        TextFormat(self.0 | DT_SINGLELINE)
    }

    /// Wrap on spaces when the line is too long.
    pub const fn word_wrap(self) -> TextFormat {
        TextFormat(self.0 | DT_WORDBREAK)
    }

    /// Replace a trailing overflow with an ellipsis.
    pub const fn end_ellipsis(self) -> TextFormat {
        TextFormat(self.0 | DT_END_ELLIPSIS)
    }

    /// Treat `&` literally instead of as an accelerator marker.
    pub const fn no_prefix(self) -> TextFormat {
        TextFormat(self.0 | DT_NOPREFIX)
    }

    /// The raw `DT_*` bits.
    pub const fn bits(self) -> u32 {
        self.0
    }
}
