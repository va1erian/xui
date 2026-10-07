//! `xui-blitz`: a web view for [xui](https://github.com/va1erian/xui) whose
//! pages are parsed, styled, laid out and painted by
//! [Blitz](https://github.com/DioxusLabs/blitz): html5ever, Servo's Stylo
//! (the Firefox style system), Taffy (block, flex, grid, floats) and Parley
//! (text). It replaces both earlier views: `xui-litehtml`'s `HtmlView` (mail
//! and help readers, which handle links themselves) and `xui-netsurf`'s
//! `NetSurfView` (a browser that follows them), with one [`BlitzView`].
//!
//! # Design
//!
//! Each view runs one **engine thread** (`engine/`) that owns the Blitz
//! document: Blitz keeps no global state, so views do not share one. The UI
//! thread sends it the node's size, scale, theme and input; the engine
//! restyles and relays out, then draws the visible part of the page with the
//! `vello_cpu` rasteriser into an RGBA [`Image`](xui_core::image::Image),
//! which the view blits through the portable canvas. Blitz draws its own
//! glyphs from font files (see [`register_font`]), so text looks the same on
//! every backend.
//!
//! Pages and their style sheets, fonts and images load through `net/`:
//! `file:`, `data:` and `about:blank` here, `http:` and `https:` through the
//! application's [`Fetcher`] (see [`set_fetcher`]), with redirects followed
//! by the view. A response the view does not display becomes a download for
//! the application's [`Downloader`]. There is no JavaScript.
//!
//! # Licence
//!
//! MIT, like the rest of xui. Blitz and its dependencies are MIT or
//! Apache-2.0, except Stylo (MPL-2.0, file-level copyleft).

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod download;
mod engine;
mod fetch;
mod fonts;
mod net;
mod view;
mod widget;

pub use crate::download::{DownloadId, DownloadInfo, DownloadSink, Downloader, set_downloader};
pub use crate::fetch::{FetchMethod, FetchRequest, FetchResponder, Fetcher, set_fetcher};
pub use crate::fonts::{FontFamilies, register_font, set_font_families};
pub use crate::view::{BlitzView, BlitzViewBuilder, BlitzViewEvent};
