#![forbid(unsafe_code)]

//! The commands toolbars, menus and key bindings share.

use xui_core::Dip;

use crate::model::{
    Align, BlockKind, CharStylePatch, DocPos, InlineImage, ListKind, ObjectId, PageSetup,
    ParaStylePatch, Wrap,
};

/// A caret movement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Motion {
    /// One grapheme left (to the start of a selection).
    Left,
    /// One grapheme right (to the end of a selection).
    Right,
    /// To the start of the previous word.
    WordLeft,
    /// To the start of the next word.
    WordRight,
    /// One line up, keeping the sticky x.
    Up,
    /// One line down, keeping the sticky x.
    Down,
    /// To the start of the line.
    LineStart,
    /// To the end of the line.
    LineEnd,
    /// To the start of the document.
    DocStart,
    /// To the end of the document.
    DocEnd,
    /// One page up.
    PageUp,
    /// One page down.
    PageDown,
}

/// One user action on the document. Each edit is a single undo step.
#[derive(Clone, Debug)]
pub enum Command {
    /// Types text over the selection (`\n` starts a paragraph).
    InsertText(String),
    /// Enter: splits the paragraph, or leaves a list at an empty item.
    InsertParagraph,
    /// Shift+Enter: a line break (U+2028) inside the paragraph.
    InsertLineBreak,
    /// Ctrl+Enter: splits the paragraph and starts its second half on a new
    /// page.
    InsertPageBreak,
    /// Backspace: deletes the selection, the grapheme before the caret, leaves
    /// a list at an item's start, or merges with the previous paragraph.
    Backspace,
    /// Delete: deletes the selection or the grapheme after the caret.
    Delete,
    /// Ctrl+Backspace.
    DeleteWordBack,
    /// Ctrl+Delete.
    DeleteWordForward,
    /// Moves the caret, or extends the selection.
    Move {
        /// Where to.
        motion: Motion,
        /// Whether to keep the anchor (Shift held).
        extend: bool,
    },
    /// Ctrl+A.
    SelectAll,
    /// Selects the word at a position (double-click).
    SelectWord(DocPos),
    /// Selects the paragraph at a position (triple-click).
    SelectParagraph(DocPos),
    /// Places the caret (click), or extends the selection to it (Shift+click).
    SetCaret {
        /// The position.
        pos: DocPos,
        /// Whether to keep the anchor.
        extend: bool,
    },
    /// Ctrl+B.
    ToggleBold,
    /// Ctrl+I.
    ToggleItalic,
    /// Ctrl+U.
    ToggleUnderline,
    /// Toggles strike-through.
    ToggleStrike,
    /// Changes character attributes of the selection (or of typed text).
    SetCharStyle(CharStylePatch),
    /// Changes paragraph attributes of the selected paragraphs.
    SetParaStyle(ParaStylePatch),
    /// Sets the alignment of the selected paragraphs.
    SetAlign(Align),
    /// Makes the selected paragraphs a list of this kind, or removes the list
    /// if they all already are one.
    ToggleList(ListKind),
    /// Raises the list level, or the left indent outside a list.
    Indent,
    /// Lowers the list level, or the left indent outside a list.
    Outdent,
    /// Sets the structural role (body, heading, quote).
    SetBlockKind(BlockKind),
    /// Changes the paper size and margins.
    SetPageSetup(PageSetup),
    /// Inserts an image over the selection and selects it.
    InsertImage(InlineImage),
    /// Changes an image's size, wrap, alt text or pixels.
    SetObject {
        /// The image.
        id: ObjectId,
        /// Its new value.
        object: InlineImage,
    },
    /// Commits an image resize (send once when the drag ends; use
    /// `EditorState::preview_object_size` for the frames in between).
    ResizeObject {
        /// The image.
        id: ObjectId,
        /// Its new displayed width and height.
        size: (Dip, Dip),
    },
    /// Moves an image's anchor to a position (a drag-move), selecting it.
    MoveObject {
        /// The image.
        id: ObjectId,
        /// The drop position, in the document before the move.
        to: DocPos,
    },
    /// Changes how text flows around an image.
    SetWrap {
        /// The image.
        id: ObjectId,
        /// The new wrap.
        wrap: Wrap,
    },
    /// Selects an image.
    SelectObject(ObjectId),
    /// Ctrl+Z.
    Undo,
    /// Ctrl+Y.
    Redo,
    /// Ctrl+X.
    Cut,
    /// Ctrl+C.
    Copy,
    /// Ctrl+V.
    Paste,
}
