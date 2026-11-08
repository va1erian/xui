#![forbid(unsafe_code)]

//! What laying out a block produces: shaped pieces of text and the
//! decorations behind them, in device pixels relative to the block's corner.

use std::sync::Arc;

use xui_core::backend::{TextLayout, TextShaper};

use super::cache::ShapeCache;
use crate::model::Document;
use crate::style::{MarkdownStyle, Role};

/// A rectangle in fractional device pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct FRect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl FRect {
    pub fn new(x: f32, y: f32, w: f32, h: f32) -> FRect {
        FRect { x, y, w, h }
    }

    pub fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.x && x < self.x + self.w && y >= self.y && y < self.y + self.h
    }
}

/// Shaped text at a place in its block.
#[derive(Clone)]
pub(crate) struct Piece {
    pub layout: Arc<dyn TextLayout>,
    pub x: f32,
    pub y: f32,
    pub role: Role,
}

/// Something painted behind the text.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Decor {
    /// A filled rectangle: code and header backgrounds, rules, quote bars,
    /// underlines and strike-throughs.
    Fill(FRect, Role),
    /// A rectangle's outline: table cells, task boxes.
    Frame(FRect, Role),
    /// A filled circle: a bullet.
    Disc(FRect, Role),
}

/// One block laid out.
#[derive(Clone, Default)]
pub(crate) struct BlockLayout {
    pub height: f32,
    pub pieces: Vec<Piece>,
    pub decor: Vec<Decor>,
    /// Where each link can be clicked, and which link it is.
    pub links: Vec<(FRect, u32)>,
    /// The area pieces are clipped to (a code block's lines do not wrap).
    pub clip: Option<FRect>,
}

/// What laying out needs, borrowed from the [`Layout`](super::Layout).
pub(crate) struct Ctx<'a> {
    pub doc: &'a Document,
    pub style: &'a MarkdownStyle,
    pub shaper: &'a dyn TextShaper,
    pub cache: &'a mut ShapeCache,
    pub dpi: u32,
}

impl Ctx<'_> {
    /// Device pixels per design unit.
    pub fn scale(&self) -> f32 {
        self.dpi as f32 / 96.0
    }

    /// A design length in device pixels.
    pub fn px(&self, dip: xui_core::Dip) -> f32 {
        dip.0 * self.scale()
    }
}
