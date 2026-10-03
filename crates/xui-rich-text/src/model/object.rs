#![forbid(unsafe_code)]

//! Anchored objects: images that live at one character of a paragraph.
//!
//! The paragraph text holds [`OBJECT_CHAR`] at the anchor and the object itself
//! lives in the [`ObjectTable`], so caret movement, deletion, undo and copy
//! treat an image like any other character.

use std::collections::HashMap;
use std::sync::Arc;

use xui_core::{Dip, Image};

/// U+FFFC OBJECT REPLACEMENT CHARACTER: the anchor of an object in the text.
pub const OBJECT_CHAR: char = '\u{FFFC}';

/// The id of an object in an [`ObjectTable`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ObjectId(pub(crate) u32);

/// Which side of the text a floating image sits on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Side {
    /// Against the left edge; text flows on its right.
    Left,
    /// Against the right edge; text flows on its left.
    Right,
}

/// How text flows around an image.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Wrap {
    /// The image sits on the baseline like a large glyph.
    Inline,
    /// The image floats to `side` and text flows beside it, `margin` away.
    Square {
        /// The side the image floats to.
        side: Side,
        /// The gap between the image and the text.
        margin: Dip,
    },
    /// The image takes a band of its own; text continues above and below.
    TopAndBottom {
        /// The gap above and below the image.
        margin: Dip,
    },
}

impl Wrap {
    /// A square wrap to `side` with the default margin.
    pub fn square(side: Side) -> Wrap {
        Wrap::Square {
            side,
            margin: Dip(8.0),
        }
    }

    /// Whether the image is laid out outside the line (not [`Wrap::Inline`]).
    pub fn is_float(self) -> bool {
        !matches!(self, Wrap::Inline)
    }
}

/// An image anchored in the text.
#[derive(Clone, Debug)]
pub struct InlineImage {
    /// The pixels.
    pub image: Arc<Image>,
    /// The displayed width and height.
    pub size: (Dip, Dip),
    /// How text flows around it.
    pub wrap: Wrap,
    /// Alternative text, used by Markdown export and accessibility.
    pub alt: String,
}

/// The objects a document's paragraphs anchor.
#[derive(Clone, Debug, Default)]
pub struct ObjectTable {
    objects: HashMap<ObjectId, InlineImage>,
    next: u32,
}

impl ObjectTable {
    /// An empty table.
    pub fn new() -> ObjectTable {
        ObjectTable::default()
    }

    /// Adds `object` and returns its new id.
    pub fn insert(&mut self, object: InlineImage) -> ObjectId {
        let id = ObjectId(self.next);
        self.next += 1;
        self.objects.insert(id, object);
        id
    }

    /// Restores `object` under a known `id` (undo, loading a file).
    pub fn insert_with_id(&mut self, id: ObjectId, object: InlineImage) {
        self.next = self.next.max(id.0 + 1);
        self.objects.insert(id, object);
    }

    /// The object `id` names.
    pub fn get(&self, id: ObjectId) -> Option<&InlineImage> {
        self.objects.get(&id)
    }

    /// The object `id` names, mutably.
    pub fn get_mut(&mut self, id: ObjectId) -> Option<&mut InlineImage> {
        self.objects.get_mut(&id)
    }

    /// Removes and returns the object `id` names.
    pub fn remove(&mut self, id: ObjectId) -> Option<InlineImage> {
        self.objects.remove(&id)
    }

    /// The number of objects.
    pub fn len(&self) -> usize {
        self.objects.len()
    }

    /// Whether the table is empty.
    pub fn is_empty(&self) -> bool {
        self.objects.is_empty()
    }
}
