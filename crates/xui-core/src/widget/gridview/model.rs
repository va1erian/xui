#![forbid(unsafe_code)]

//! The grid's data model ([`GridModel`]), its borrowed per-tile data
//! ([`Tile`]), the tile-size spec ([`TileSize`]) and the paint context
//! ([`TilePaint`]) handed to a custom tile painter.

use crate::geometry::Rect;
use crate::image::Image;
use crate::theme::Theme;
use crate::units::Dip;

/// One tile's data, borrowed from the model for the duration of a paint.
///
/// Build it with [`Tile::new`] and attach art with [`Tile::image`].
pub struct Tile<'a> {
    /// The tile caption.
    pub label: &'a str,
    /// Optional art, drawn by the default painter.
    pub image: Option<&'a Image>,
}

impl<'a> Tile<'a> {
    /// A caption-only tile.
    pub fn new(label: &'a str) -> Tile<'a> {
        Tile { label, image: None }
    }

    /// A caption tile with art.
    pub fn with_image(label: &'a str, image: &'a Image) -> Tile<'a> {
        Tile {
            label,
            image: Some(image),
        }
    }

    /// Attaches art to this tile.
    pub fn image(mut self, image: &'a Image) -> Tile<'a> {
        self.image = Some(image);
        self
    }
}

/// Supplies a [`GridView`](super::GridView) with tiles without storing them in
/// the view itself.
///
/// Only the visible tiles are read on each paint, so a model of any size costs
/// the same to draw. The model is borrowed and [`tile`](GridModel::tile)
/// returns borrowed data, so a virtual grid allocates nothing per visible tile.
pub trait GridModel {
    /// The number of tiles.
    fn len(&self) -> usize;

    /// The tile at `index`, or `None` past the end.
    fn tile(&self, index: usize) -> Option<Tile<'_>>;

    /// Whether the model holds no tiles.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl GridModel for Vec<String> {
    fn len(&self) -> usize {
        Vec::len(self)
    }

    fn tile(&self, index: usize) -> Option<Tile<'_>> {
        self.get(index).map(|label| Tile::new(label))
    }
}

/// A tile's design size and the gap between neighbours.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TileSize {
    /// Tile width.
    pub width: Dip,
    /// Tile height.
    pub height: Dip,
    /// Gap between tiles, on both axes.
    pub gap: Dip,
}

impl TileSize {
    /// A `width` by `height` tile with the default gap.
    pub fn new(width: Dip, height: Dip) -> TileSize {
        TileSize {
            width,
            height,
            ..TileSize::default()
        }
    }

    /// Sets the gap between tiles.
    pub fn gap(mut self, gap: Dip) -> TileSize {
        self.gap = gap;
        self
    }
}

impl Default for TileSize {
    fn default() -> TileSize {
        TileSize {
            width: Dip(128.0),
            height: Dip(152.0),
            gap: Dip(8.0),
        }
    }
}

impl From<Dip> for TileSize {
    /// A square tile with the default gap.
    fn from(size: Dip) -> TileSize {
        TileSize::new(size, size)
    }
}

impl From<(Dip, Dip)> for TileSize {
    /// A tile with the default gap.
    fn from((width, height): (Dip, Dip)) -> TileSize {
        TileSize::new(width, height)
    }
}

/// Everything a custom tile painter draws from: the borrowed [`Tile`], the
/// tile's device-pixel rectangle and its current state.
///
/// The widget paints the selection/hover fill behind the painter, so a painter
/// only draws the tile's own content.
pub struct TilePaint<'a> {
    /// The tile's model data.
    pub tile: Tile<'a>,
    /// The tile rectangle, in device pixels.
    pub rect: Rect,
    /// The tile's index in the model.
    pub index: usize,
    /// Whether this tile is the selection.
    pub selected: bool,
    /// Whether the pointer is over this tile.
    pub hovered: bool,
    /// The window's theme.
    pub theme: &'a Theme,
    /// The window's dots-per-inch.
    pub dpi: u32,
}
