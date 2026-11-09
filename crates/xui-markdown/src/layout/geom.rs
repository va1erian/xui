#![forbid(unsafe_code)]

//! What laying out a block produces: shaped pieces of text and the
//! decorations behind them, in device pixels relative to the block's corner.

use std::ops::Range;
use std::sync::Arc;

use xui_core::backend::{TextLayout, TextShaper};
use xui_core::image::Image;

use super::cache::ShapeCache;
use crate::highlight::Highlighter;
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
    /// The bytes of [`Document::text`] it shows, for hit-testing and
    /// selection; empty for a list marker, which is not text of the document.
    pub range: Range<u32>,
    /// Whether its tabs were widened to spaces for shaping (code), so its
    /// shaped bytes differ from the document's.
    pub tabs: bool,
    pub role: Role,
}

impl Piece {
    /// The document position of byte `local` of the piece's shaped text;
    /// `text` is [`Document::text`].
    pub fn offset(&self, text: &str, local: usize) -> u32 {
        let local = if self.tabs {
            super::code::original_offset(&text[self.slice()], local)
        } else {
            local
        };
        (self.range.start + local as u32).min(self.range.end)
    }

    /// The byte of the piece's shaped text at document position `pos`
    /// (clamped to [`Piece::range`]); `text` is [`Document::text`].
    pub fn local(&self, text: &str, pos: u32) -> usize {
        let local = (pos.clamp(self.range.start, self.range.end) - self.range.start) as usize;
        if self.tabs {
            super::code::expanded_offset(&text[self.slice()], local)
        } else {
            local
        }
    }

    fn slice(&self) -> Range<usize> {
        self.range.start as usize..self.range.end as usize
    }
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

/// An image placed in its block.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ImageBox {
    pub rect: FRect,
    /// Which image, an index into [`Document::images`].
    pub image: u32,
    /// The bytes of [`Document::text`] standing for it (its alt text).
    pub range: Range<u32>,
    /// The line it stands on, across its flow's width (a table cell's, in a
    /// table): a point beside its line's text is on that line.
    pub line: FRect,
}

/// One block laid out.
#[derive(Clone, Default)]
pub(crate) struct BlockLayout {
    pub height: f32,
    pub pieces: Vec<Piece>,
    pub decor: Vec<Decor>,
    /// The images, drawn over the decorations.
    pub images: Vec<ImageBox>,
    /// Where each link can be clicked, and which link it is.
    pub links: Vec<(FRect, u32)>,
    /// The area pieces are clipped to (a code block's, for lines that do not
    /// wrap).
    pub clip: Option<FRect>,
}

/// What laying out needs, borrowed from the [`Layout`](super::Layout).
pub(crate) struct Ctx<'a> {
    pub doc: &'a Document,
    pub style: &'a MarkdownStyle,
    pub shaper: &'a dyn TextShaper,
    pub cache: &'a mut ShapeCache,
    pub dpi: u32,
    /// The app's code highlighter, if it gave one.
    pub highlighter: Option<&'a dyn Highlighter>,
    /// Per image of the document: its pixels, once the app gave them.
    pub images: &'a [Option<Arc<Image>>],
    /// The images laid out without pixels, for the app to be asked for.
    pub wanted: &'a mut Vec<u32>,
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
