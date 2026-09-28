//! `xui-litehtml` — an HTML view that lays a page out with
//! [litehtml](https://github.com/litehtml/litehtml) (through
//! `va1erian/litehtml-rs`) and paints it through the portable
//! [xui-core](https://github.com/va1erian/xui) `Canvas`, so it runs on any
//! backend that provides a text shaper.
//!
//! **Render, scroll, resize, DPI, links, text selection, copy, keyboard.** It
//! was extracted from esMail's `litehtml-view-d2d`. [`HtmlView`] is a
//! custom-painted portable node: the app gives it bounds and maps its
//! [`HtmlViewEvent`]s to its own `Msg`. Copy to the clipboard still uses
//! `xui-win32`, so the crate stays Windows-only for now.
//!
//! # Design
//!
//! A render **worker thread** owns the `!Send` litehtml `Document` and its
//! container, exactly like `egui-litehtml-webview`. The UI thread sends render
//! jobs (HTML + a layout width in DIPs + a job id); superseded jobs are dropped
//! and the UI ignores frames that are not the newest.
//!
//! The container measures text with the backend's portable `TextShaper` and
//! turns every draw callback into a backend-neutral [`Cmd`] — a display list of
//! rects, rounded rects, border edges, gradients, images and text runs — with
//! its own `Point`/`Rect`/`Rgba` types (`geom.rs`). The [`DisplayList`] crosses
//! to the UI thread as an `Arc`; [`Painter::paint`] replays it into a
//! portable `Canvas`, culling to the visible region and applying the scroll offset, so
//! a tall newsletter costs only what is on screen.
//!
//! # Links and selection without a `Document`
//!
//! The draw pass also records a [`LinkTable`] and a [`TextRunTable`] (one run
//! per word: box, text, per-character x offsets, containing block, forced line
//! breaks) and ships them with the frame. Link clicks, the hover cursor, the
//! selection highlight, and copy are then point/geometry lookups on the UI
//! thread, with no second parse + layout. The character boundary under the
//! pointer and the highlight boxes come from the shaper's own hit-testing and
//! selection rects (a layout rebuilt per run), so right-to-left and complex
//! text select accurately — the table's left-to-right offsets are only a
//! fallback and what the pure selection logic is tested against.
//!
//! # Units
//!
//! Everything is laid out in **device-independent pixels** (DIPs); the painter
//! scales to the canvas's DPI, so text stays crisp at any scale and
//! nothing re-lays-out on a DPI change except when the width in DIPs changes.
//!
//! # Images
//!
//! `data:` URIs are decoded with the `image` crate on the worker into
//! `Arc<Image>`; paint draws them via `Canvas::draw_image`. Remote images are
//! fetched through the host's [`ImageFetcher`], when it installed one, and
//! otherwise left unloaded.
//!
//! # On other targets
//!
//! This crate compiles to an empty crate on non-Windows so the workspace's
//! Linux CI stays green.

#![cfg(windows)]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod container;
mod engine;
mod geom;
mod links;
mod list;
mod paint;
mod selection;
mod text;
mod text_runs;
mod view;
mod widget;
mod worker;

pub use crate::geom::{Point, Radius, Rect, Rgba};
pub use crate::links::{Link, LinkTable};
pub use crate::list::{
    BorderKind, BorderPaint, Cmd, Dash, DisplayList, EdgePaint, FontDesc, FontKey, GradientStop,
    Image, ImageKey, LinearGradient, RadialGradient, Stroke, decompose_borders, normalize_stops,
};
pub use crate::paint::Painter;
pub use crate::selection::{Selection, TextPos};
pub use crate::text::TextSystem;
pub use crate::text_runs::{TextRun, TextRunTable};
pub use crate::view::{HtmlView, HtmlViewEvent};
pub use crate::worker::ImageFetcher;
