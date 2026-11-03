#![forbid(unsafe_code)]
#![warn(missing_docs)]

//! A multi-colour vector icon set for xui: 36 icons in four [`Category`]s
//! (hardware, files, system and toolkit), each an [`Icon`] variant.
//!
//! The crate is optional and depends on [`xui_core`] only. Icons are drawn with
//! [`draw`] through the portable `Canvas` path API, so the software backend
//! renders them with `tiny-skia`, at any size and DPI.
//!
//! The artwork is "Global Village": 90s world-music flat colour with chunky ink
//! outlines and woven zig-zag trim, recolourable with a [`Palette`].
//!
//! ```no_run
//! use xui_icons::{Icon, Palette, draw};
//! # fn paint(canvas: &mut dyn xui_core::backend::Canvas, rect: xui_core::Rect) {
//! draw(canvas, Icon::Network, rect, &Palette::GLOBAL_VILLAGE);
//! # }
//! ```
//!
//! The vendored SVGs, and PNG and `.ico` exports of them, live in
//! `assets/village/`; `assets/generate.py` compiles the SVGs into Rust.

mod draw;
mod icon;
mod shape;
mod tone;
mod village;

use village as style;

#[cfg(test)]
mod tests;

pub use draw::{GRID, draw};
pub use icon::{Category, Icon};
pub use shape::{Line, Paint, Shape};
pub use tone::{Palette, Tone};
