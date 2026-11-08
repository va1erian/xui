#![forbid(unsafe_code)]
#![warn(missing_docs)]

//! A fast, small, read-only Markdown view for
//! [xui](https://github.com/va1erian/xui).
//!
//! CommonMark with GFM tables, strike-through and task lists is parsed into a
//! flat [`Document`], laid out a block at a time (the visible blocks first)
//! and painted from cached layouts. The crate is portable: it shapes text only
//! through `xui-core`'s [`TextShaper`](xui_core::backend::TextShaper) and
//! paints only through the [`Canvas`](xui_core::backend::Canvas).
//!
//! See `docs/plans/markdown-view.md` for the design.

mod copy;
pub mod highlight;
mod layout;
pub mod model;
mod parse;
mod style;
mod view;

pub use highlight::{CodeClass, CodeSpan, Highlighter};
pub use model::{Align, Block, BlockKind, Document, Inline, Marker, Span, Table};
pub use style::{CodeColors, CodePalette, MarkdownStyle};
pub use view::MarkdownView;
