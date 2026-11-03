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
//! - `icons` adds [`xui_icons`], the optional Global Village icon set.
//! - `rich-text` adds [`xui_rich_text`], the editable rich-text widget.
//! - `form` adds [`form`], declarative `.lfm` forms built into widgets.
//! - `rhai` adds [`script`], forms whose events run Rhai handlers (and
//!   implies `form`).
//!
//! Both backends run the exact same widgets. An app starts with [`app`],
//! which picks the backend (and renders headless screenshots when
//! `XUI_SNAPSHOT` is set); `xui_core::run_app` takes an explicit one. See
//! [Backends](https://github.com/va1erian/xui/blob/main/docs/backends.md).

pub use xui_core;
pub use xui_core::*;

#[cfg(feature = "canvas")]
mod launch;
#[cfg(feature = "canvas")]
pub use launch::{Launch, app, default_backend};

/// Everything an app needs in one `use`: the widget builders, layouts,
/// handles and widget types, and [`app`].
pub mod prelude {
    pub use xui_core::prelude::*;

    #[cfg(feature = "canvas")]
    pub use crate::app;
}

#[cfg(feature = "canvas")]
pub use xui_canvas;

#[cfg(feature = "icons")]
pub use xui_icons;
#[cfg(feature = "rich-text")]
pub use xui_rich_text;

/// Declarative forms: the `.lfm` document, its schema, validation and the
/// builder that turns a form into live widgets.
#[cfg(feature = "form")]
pub use xui_form as form;
/// Rhai-scripted forms: `fn <control>_<event>` handlers run against a live
/// form.
#[cfg(feature = "rhai")]
pub use xui_script as script;

#[cfg(all(feature = "d2d", windows))]
pub use xui_win32;
