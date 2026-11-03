#![forbid(unsafe_code)]

//! Re-exported from [`xui_core`]. The layout arithmetic is pure and lives in
//! the core so every backend shares it; this module keeps the crate's existing
//! paths working.

pub use xui_core::layout::*;
