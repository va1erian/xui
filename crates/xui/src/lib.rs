#![forbid(unsafe_code)]

//! Cross-platform UI for Rust: **small, fast, idiomatic and properly themed**.
//!
//! `xui` is the umbrella crate. The portable front layer lives in
//! [`xui_core`]; a backend is selected by feature:
//!
//! - `win32` (default) re-exports the Win32 backend, [`xui_win32`].
//! - `canvas` is the planned software-rendered backend and enables nothing yet.
//!
//! With the default feature on Windows the win32 widget layer (windows,
//! controls, layout, theming) is available directly from `xui`, so an app
//! writes `use xui::prelude::*;` and never names the backend. On other targets
//! only the portable [`xui_core`] types are re-exported until `canvas` lands.

pub use xui_core;

#[cfg(all(feature = "win32", windows))]
pub use xui_win32;

#[cfg(all(feature = "win32", windows))]
pub use xui_win32::*;

#[cfg(not(all(feature = "win32", windows)))]
pub use xui_core::*;
