#![forbid(unsafe_code)]

//! The small value types a form file is written with: lengths, alignment,
//! anchors and grid tracks, each mapping to its `xui-core` counterpart.

use std::fmt;

use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use xui_core::layout;
use xui_core::units::Dip;

/// A length in design units. A whole number is written without a fraction
/// (`16`, not `16.0`), so hand-written and saved files look the same.
#[derive(Clone, Copy, Debug, Default, PartialEq, PartialOrd)]
pub struct Length(pub f32);

impl Length {
    /// The length as a [`Dip`].
    pub fn dip(self) -> Dip {
        Dip(self.0)
    }
}

impl From<f32> for Length {
    fn from(value: f32) -> Length {
        Length(value)
    }
}

impl From<i32> for Length {
    fn from(value: i32) -> Length {
        Length(value as f32)
    }
}

impl Serialize for Length {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if self.0.fract() == 0.0 && self.0.abs() < 1e9 {
            serializer.serialize_i64(self.0 as i64)
        } else {
            serializer.serialize_f32(self.0)
        }
    }
}

impl<'de> Deserialize<'de> for Length {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Length, D::Error> {
        struct LengthVisitor;

        impl Visitor<'_> for LengthVisitor {
            type Value = Length;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a length in design units")
            }

            fn visit_i64<E: de::Error>(self, value: i64) -> Result<Length, E> {
                Ok(Length(value as f32))
            }

            fn visit_u64<E: de::Error>(self, value: u64) -> Result<Length, E> {
                Ok(Length(value as f32))
            }

            fn visit_f64<E: de::Error>(self, value: f64) -> Result<Length, E> {
                Ok(Length(value as f32))
            }
        }

        deserializer.deserialize_any(LengthVisitor)
    }
}

/// Where an entry sits within the space its container gives it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Align {
    /// At the start (left or top), at its natural size.
    Start,
    /// Centred, at its natural size.
    Center,
    /// At the end (right or bottom), at its natural size.
    End,
    /// Across the whole space.
    Stretch,
}

impl From<Align> for layout::Align {
    fn from(align: Align) -> layout::Align {
        match align {
            Align::Start => layout::Align::Start,
            Align::Center => layout::Align::Center,
            Align::End => layout::Align::End,
            Align::Stretch => layout::Align::Stretch,
        }
    }
}

/// How an entry of an `Absolute` layout follows the layout as it grows or
/// shrinks from its design size.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Anchor {
    /// Pinned to the top-left corner: never moves.
    TopLeft,
    /// Centred horizontally, pinned to the top.
    Top,
    /// Pinned to the top-right corner.
    TopRight,
    /// Centred vertically, pinned to the left.
    Left,
    /// Kept at its offset from the centre.
    Center,
    /// Centred vertically, pinned to the right.
    Right,
    /// Pinned to the bottom-left corner.
    BottomLeft,
    /// Centred horizontally, pinned to the bottom.
    Bottom,
    /// Pinned to the bottom-right corner.
    BottomRight,
    /// Left and right edges pinned: the width grows with the layout.
    StretchHorizontal,
    /// Top and bottom edges pinned: the height grows with the layout.
    StretchVertical,
    /// All four edges pinned.
    Fill,
}

impl From<Anchor> for layout::Anchor {
    fn from(anchor: Anchor) -> layout::Anchor {
        match anchor {
            Anchor::TopLeft => layout::Anchor::TopLeft,
            Anchor::Top => layout::Anchor::Top,
            Anchor::TopRight => layout::Anchor::TopRight,
            Anchor::Left => layout::Anchor::Left,
            Anchor::Center => layout::Anchor::Center,
            Anchor::Right => layout::Anchor::Right,
            Anchor::BottomLeft => layout::Anchor::BottomLeft,
            Anchor::Bottom => layout::Anchor::Bottom,
            Anchor::BottomRight => layout::Anchor::BottomRight,
            Anchor::StretchHorizontal => layout::Anchor::StretchHorizontal,
            Anchor::StretchVertical => layout::Anchor::StretchVertical,
            Anchor::Fill => layout::Anchor::Fill,
        }
    }
}

/// One column of a `Grid`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Track {
    /// As wide as the widest single-column entry in it.
    Auto,
    /// Exactly this many design units.
    Fixed(Length),
    /// A share of the leftover width, by weight.
    Fill(u32),
}

impl From<Track> for layout::Track {
    fn from(track: Track) -> layout::Track {
        match track {
            Track::Auto => layout::Track::Auto,
            Track::Fixed(width) => layout::Track::Fixed(width.dip()),
            Track::Fill(weight) => layout::Track::Fill(weight),
        }
    }
}

/// A separator's direction.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Orientation {
    /// A horizontal rule, for a column.
    #[default]
    Horizontal,
    /// A vertical rule, for a row.
    Vertical,
}
