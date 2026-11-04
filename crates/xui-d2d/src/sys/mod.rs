//! The only module with `unsafe`: raw Win32, Direct2D and DirectWrite calls,
//! wrapped in a safe API the rest of the crate uses. Nothing here names an xui
//! widget; the backend modules above translate.

pub(crate) mod clipboard;
pub(crate) mod dialog;
pub(crate) mod gfx;
pub(super) mod message_window;
mod pump;
pub(crate) mod settings;
pub(crate) mod window;
