//! `xui-netsurf`: a web view for [xui](https://github.com/va1erian/xui) whose
//! pages are laid out by the [NetSurf](https://www.netsurf-browser.org/)
//! browser core, as a prototype to compare against `xui-litehtml`.
//!
//! # Design
//!
//! NetSurf's core keeps global state, so the process runs one **engine
//! thread** (`engine.rs`) that serves every [`NetSurfView`]. NetSurf asks the
//! host to measure text (`fonts.rs`, over the backend's portable shaper) and
//! draws through plotter callbacks, which `record.rs` turns into an
//! `xui-litehtml` [`DisplayList`](xui_litehtml::DisplayList). The list crosses
//! to the UI thread, where the view paints it with the same
//! [`Painter`](xui_litehtml::Painter) a litehtml page uses: the two engines
//! differ only in layout.
//!
//! NetSurf follows links, redirects and forms itself. It fetches `file:`,
//! `data:`, `about:` and `resource:` URLs on its own, and `http:` and
//! `https:` through the application's [`Fetcher`] (see [`set_fetcher`]), so
//! the HTTP client and its TLS are the application's choice. NetSurf decodes
//! GIF and BMP images; PNG and JPEG are decoded in Rust (`image.rs`). There
//! is no JavaScript.
//!
//! # Licence
//!
//! NetSurf is GPL-2.0-only, so this crate is too, unlike the MIT crates of the
//! main workspace; it is its own cargo workspace for that reason.
//!
//! The only `unsafe` code is in `sys`, the boundary to `netsurf-sys`.

#![warn(missing_docs)]

mod download;
mod engine;
mod families;
mod fetch;
mod fonts;
mod image;
mod pointer;
mod record;
mod sys;
mod view;
mod widget;

pub use crate::download::{DownloadId, DownloadInfo, DownloadSink, Downloader, set_downloader};
pub use crate::families::{FontFamilies, set_font_families};
pub use crate::fetch::{FetchMethod, FetchRequest, FetchResponder, Fetcher, set_fetcher};
pub use crate::view::{NetSurfView, NetSurfViewEvent};
