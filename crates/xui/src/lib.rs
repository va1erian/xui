#![forbid(unsafe_code)]
#![warn(missing_docs)]

//! Cross-platform UI for Rust: **small, fast, idiomatic and properly themed**.
//!
//! `xui` is the umbrella crate. The portable front layer lives in
//! [`xui_core`]; a backend is selected by feature:
//!
//! - `win32` (default) re-exports the Win32 backend, [`xui_win32`].
//! - `canvas` re-exports the cross-platform software backend, [`xui_canvas`]
//!   (a `winit` window + `tiny-skia` rendering).
//!
//! With the default feature on Windows the win32 widget layer (windows,
//! controls, layout, theming) is available directly from `xui`, so an app
//! writes `use xui::prelude::*;` and never names the backend. With `canvas` the
//! same portable widgets run on every platform through [`xui_canvas::WinitBackend`].

pub use xui_core;

#[cfg(feature = "canvas")]
pub use xui_canvas;

#[cfg(all(feature = "win32", windows))]
pub use xui_win32;

#[cfg(all(feature = "win32", windows))]
pub use xui_win32::*;

#[cfg(not(all(feature = "win32", windows)))]
pub use xui_core::*;
