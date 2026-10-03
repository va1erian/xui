#![forbid(unsafe_code)]

//! The editing controller: everything the widget does to a document in
//! response to input, with no window, painting or layout of its own.
//!
//! [`EditorState`] owns the document, its history and the selection.
//! [`EditorState::exec`] runs one [`Command`] (typing, deleting, moving the
//! caret, formatting, clipboard, undo) as one undoable step and returns an
//! [`Effect`] saying what the view must refresh. Motions that depend on line
//! layout ask a [`LineNav`], which the layout implements.

mod clipboard;
mod command;
mod controller;
mod format;
pub mod handles;
mod motion;
mod nav;
mod objects;
mod table;
mod text;
mod transfer;
mod tx;

pub use clipboard::{Clipboard, MemoryClipboard};
pub use command::{Command, Motion};
pub use controller::{EditorState, Effect};
pub use handles::{HANDLE_SIZE, Handle, Handles, MIN_SIZE, resize};
pub use nav::LineNav;
pub use table::TableCursor;
