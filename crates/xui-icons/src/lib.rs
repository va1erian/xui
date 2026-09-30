#![forbid(unsafe_code)]
#![warn(missing_docs)]

//! A multi-colour vector icon set for xui: 36 icons in four [`Category`]s
//! (hardware, files, system and toolkit), each an [`Icon`] variant.
//!
//! The crate is optional and depends on [`xui_core`] only. Icons are drawn with
//! [`draw`] through the portable `Canvas` path API, so the software backend
//! renders them with `tiny-skia`, at any size and DPI.
//!
//! # Styles
//!
//! The same icons come in two looks, and the one you build with is the only
//! one compiled in ([`STYLE`] says which):
//!
//! - [`Style::GlobalVillage`] (default): 90s world-music flat colour with chunky
//!   ink outlines and woven zig-zag trim. Recolourable with a [`Palette`].
//! - [`Style::Aero`] (`--features aero`): glossy Vista-era glass with gradients,
//!   specular highlights and soft shadows. Needs a backend with
//!   `Canvas::fill_path_linear` for its gradients.
//!
//! ```no_run
//! use xui_icons::{Icon, Palette, draw};
//! # fn paint(canvas: &mut dyn xui_core::backend::Canvas, rect: xui_core::Rect) {
//! draw(canvas, Icon::Network, rect, &Palette::GLOBAL_VILLAGE);
//! # }
//! ```
//!
//! The vendored SVGs, and PNG and `.ico` exports of them, live in
//! `assets/<style>/`; `assets/generate.py` compiles the SVGs into Rust.

#[cfg(feature = "aero")]
mod aero;
mod draw;
mod gradient;
mod icon;
mod shape;
mod tone;
#[cfg(not(feature = "aero"))]
mod village;

#[cfg(feature = "aero")]
use aero as style;
#[cfg(not(feature = "aero"))]
use village as style;

#[cfg(test)]
mod tests;

pub use draw::{GRID, draw};
pub use gradient::{Gradient, MAX_STOPS};
pub use icon::{Category, Icon};
pub use shape::{Line, Paint, Shape};
pub use tone::{Palette, Tone};

/// The artwork compiled into this build.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Style {
    /// Flat 90s world-music colour with ink outlines.
    GlobalVillage,
    /// Glossy Vista-era glass with gradients.
    Aero,
}

/// The [`Style`] this build draws: [`Style::Aero`] with the `aero` feature,
/// otherwise [`Style::GlobalVillage`].
pub const STYLE: Style = if cfg!(feature = "aero") {
    Style::Aero
} else {
    Style::GlobalVillage
};
