#![forbid(unsafe_code)]

//! Re-exported from [`xui_core`]. The types live in the core so every backend
//! shares one definition; this module keeps the crate's existing paths working.

pub use xui_core::geometry::*;
