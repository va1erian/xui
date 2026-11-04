//! Platform boundary of the canvas backend. Every `unsafe` block in the crate
//! lives under this module: [`gl`] wraps `glutin`'s context creation and the
//! `glow` loader behind a safe API, [`double_click`] reads the system's
//! double-click thresholds, [`animation`] its animation preference and
//! [`child_window`] parents a `winit` window to another.

pub(crate) mod animation;
pub(crate) mod child_window;
pub(crate) mod double_click;
pub(crate) mod gl;
