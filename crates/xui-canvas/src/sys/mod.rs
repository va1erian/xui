//! Platform boundary of the canvas backend. Every `unsafe` block in the crate
//! lives under [`gl`], which wraps `glutin`'s context creation and the `glow`
//! loader behind a safe API.

pub(crate) mod gl;
