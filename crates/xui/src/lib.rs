#![forbid(unsafe_code)]
#![warn(missing_docs)]

//! Cross-platform UI for Rust: **small, fast, idiomatic and properly themed**.
//!
//! `xui` is the umbrella crate: its bare names are always the portable widget
//! layer, [`xui_core`], which is re-exported at the crate root. A backend is
//! selected by feature and passed to [`xui_core::run_app`]:
//!
//! - `canvas` (default) is the cross-platform software backend, [`xui_canvas`]
//!   (a `winit` window + `tiny-skia` rendering). Builds and runs on every
//!   platform.
//! - `d2d` is the Windows-only backend, [`xui_win32`]: native window chrome
//!   with Direct2D/DirectWrite painting (GDI fallback), for apps that want
//!   higher fidelity or performance on Windows specifically.
//!
//! Both backends run the exact same widgets; an app written against
//! `xui::prelude::*` and `xui_core::run_app` moves between them by swapping
//! which backend it constructs. See [Backends](https://github.com/va1erian/xui/blob/main/docs/backends.md).

pub use xui_core;
pub use xui_core::*;

#[cfg(feature = "canvas")]
pub use xui_canvas;

#[cfg(all(feature = "d2d", windows))]
pub use xui_win32;
