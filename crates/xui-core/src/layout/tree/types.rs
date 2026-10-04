#![forbid(unsafe_code)]

//! The values the layout tree is built from and asks its leaves for.

use crate::geometry::Size;
use crate::layout::Insets;
use crate::units::Dip;

/// What the tree asks about a leaf when it lays out.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Leaf {
    /// The leaf's natural size within the constraints it was asked with, in
    /// device pixels.
    pub natural: Size,
    /// Whether the leaf takes part in layout. A hidden leaf takes no space.
    pub visible: bool,
    /// For a leaf that frames content (a group box), the insets between its
    /// edge and that content.
    pub content: Insets,
}

impl Leaf {
    /// A visible leaf of `natural` size that frames nothing.
    pub const fn new(natural: Size) -> Leaf {
        Leaf {
            natural,
            visible: true,
            content: Insets::new(Dip(0.0), Dip(0.0), Dip(0.0), Dip(0.0)),
        }
    }
}

/// The space a leaf is measured in: the most it may take on each axis, in
/// device pixels (`None` is unbounded), at a DPI.
///
/// A column measures its children with its own width as the bound, so text
/// that wraps reports its height for that width.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Constraints {
    /// The widest the leaf may be.
    pub max_width: Option<i32>,
    /// The tallest the leaf may be.
    pub max_height: Option<i32>,
    /// The window's dots per inch.
    pub dpi: u32,
}

impl Constraints {
    /// No bound on either axis.
    pub const fn unbounded(dpi: u32) -> Constraints {
        Constraints {
            max_width: None,
            max_height: None,
            dpi,
        }
    }

    /// These constraints with the width bounded to `width`.
    pub const fn with_width(mut self, width: i32) -> Constraints {
        self.max_width = Some(width);
        self
    }

    /// These constraints with the height bounded to `height`.
    pub const fn with_height(mut self, height: i32) -> Constraints {
        self.max_height = Some(height);
        self
    }

    /// These constraints less `insets` on every bounded axis.
    pub(super) fn shrink(self, insets: Insets) -> Constraints {
        let px = |value: Dip| value.to_px(self.dpi).value();
        Constraints {
            max_width: self
                .max_width
                .map(|w| (w - px(insets.left) - px(insets.right)).max(0)),
            max_height: self
                .max_height
                .map(|h| (h - px(insets.top) - px(insets.bottom)).max(0)),
            dpi: self.dpi,
        }
    }
}

/// The callback the tree asks about each leaf.
pub type LeafFn<'a, K> = &'a dyn Fn(&K, Constraints) -> Leaf;

/// How an [`Item`] is sized: along its parent's main axis, or, for
/// [`Width`](Sizing::Width) and [`Height`](Sizing::Height), along a named axis.
/// In a [`Group::grid`], `Fill`, `Fixed` and `Height` size the item's row.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Sizing {
    /// The natural size along the main axis.
    Auto,
    /// Exactly this many design units along the main axis.
    Fixed(Dip),
    /// At least this many design units along the main axis; shrinks only when
    /// the parent is too small to honour every item.
    Min(Dip),
    /// A share of the leftover main-axis space, proportional to its weight.
    Fill(u32),
    /// Exactly this many design units of width, whichever axis that is.
    Width(Dip),
    /// Exactly this many design units of height, whichever axis that is.
    Height(Dip),
}

/// Where an item sits in the space its parent gives it on an axis.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Align {
    /// At the start (left or top), at its natural size.
    Start,
    /// Centred, at its natural size.
    Center,
    /// At the end (right or bottom), at its natural size.
    End,
    /// Across the whole space.
    #[default]
    Stretch,
}

/// One column of a [`Group::grid`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Track {
    /// As wide as the widest single-column item in it.
    Auto,
    /// Exactly this many design units.
    Fixed(Dip),
    /// A share of the leftover width, proportional to its weight.
    Fill(u32),
}
