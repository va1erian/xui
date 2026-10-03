#![forbid(unsafe_code)]
#![warn(missing_docs)]

//! An editable rich-text widget for [xui](https://github.com/va1erian/xui):
//! styled runs, paragraph formatting, lists, inline images and floating images
//! that text flows around.
//!
//! The crate is portable: it shapes text only through `xui-core`'s
//! [`TextShaper`](xui_core::backend::TextShaper) and paints only through the
//! [`Canvas`](xui_core::backend::Canvas), so it runs unchanged on every
//! backend. It is built in three layers, each testable without a window:
//!
//! * [`model`]: the document, its styles and objects, and invertible edits.
//! * [`layout`]: the flow engine that breaks paragraphs into lines around
//!   floating images.
//! * [`view`]: the [`RichTextEditor`] widget that paints a laid-out document.
//! * [`format`]: the JSON save format and Markdown export.
//!
//! See `docs/plans/rich-text-editor.md` for the design.

pub mod edit;
pub mod format;
pub mod layout;
pub mod model;
pub mod view;

pub use layout::Layout;
pub use model::{DocPos, Document};
pub use view::RichTextEditor;
