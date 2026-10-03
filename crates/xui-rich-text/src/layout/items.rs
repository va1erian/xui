#![forbid(unsafe_code)]

//! Items: a paragraph's pieces shaped and measured, ready for line breaking.

use std::ops::Range;
use std::sync::Arc;

use xui_core::backend::{TextLayout, TextShaper};

use super::resolve::{baseline_shift, font_spec};
use super::segment::{Brk, SegKind, segment};
use super::shape_cache::ShapeCache;
use crate::model::{BlockKind, CharStyleId, Document, ObjectId, Paragraph, Wrap};

/// A tab is this many spaces wide.
const TAB_SPACES: f32 = 4.0;

/// What an item is.
#[derive(Clone)]
pub(crate) enum ItemKind {
    Text(Arc<dyn TextLayout>),
    Space {
        unit: f32,
    },
    Object {
        id: ObjectId,
        height: f32,
    },
    Float {
        id: ObjectId,
        size: (f32, f32),
        wrap: Wrap,
    },
    Break,
}

/// A measured piece of a paragraph.
#[derive(Clone)]
pub(crate) struct Item {
    pub range: Range<usize>,
    pub kind: ItemKind,
    pub width: f32,
    pub ascent: f32,
    pub descent: f32,
    pub dy: f32,
    pub brk: Brk,
    pub style: CharStyleId,
    pub size: f32,
}

impl Item {
    /// Whether the item is spaces (which may hang past the line's edge).
    pub fn is_space(&self) -> bool {
        matches!(self.kind, ItemKind::Space { .. })
    }
}

/// Shapes and measures the pieces of paragraphs of one block kind.
pub(crate) struct ShapeCtx<'a> {
    pub shaper: &'a dyn TextShaper,
    pub cache: &'a mut ShapeCache,
    pub doc: &'a Document,
    pub dpi: u32,
    pub scale: f32,
    pub kind: BlockKind,
    pub tick: u64,
}

impl ShapeCtx<'_> {
    /// The (ascent, descent) of an empty line in `style`.
    pub fn strut(&mut self, style: CharStyleId) -> (f32, f32) {
        let (doc, kind, shaper) = (self.doc, self.kind, self.shaper);
        self.cache
            .entry(self.dpi, style, kind, || {
                font_spec(doc.styles().char(style), kind)
            })
            .strut(shaper)
    }

    /// A shaped word: `text[range]` in `style`.
    pub fn text_item(
        &mut self,
        text: &str,
        range: Range<usize>,
        style: CharStyleId,
        brk: Brk,
    ) -> Item {
        let (doc, kind, shaper, scale) = (self.doc, self.kind, self.shaper, self.scale);
        let char_style = doc.styles().char(style);
        let entry = self
            .cache
            .entry(self.dpi, style, kind, || font_spec(char_style, kind));
        let layout = entry.word(shaper, &text[range.clone()], self.tick);
        let dy = baseline_shift(char_style, entry.spec().size, scale);
        let baseline = layout.baseline();
        Item {
            width: layout.width(),
            ascent: baseline - dy,
            descent: (layout.height() - baseline + dy).max(0.0),
            dy,
            size: entry.spec().size.0 * scale,
            kind: ItemKind::Text(layout),
            range,
            brk,
            style,
        }
    }

    fn space_item(
        &mut self,
        text: &str,
        range: Range<usize>,
        style: CharStyleId,
        brk: Brk,
    ) -> Item {
        let (doc, kind, shaper, scale) = (self.doc, self.kind, self.shaper, self.scale);
        let entry = self.cache.entry(self.dpi, style, kind, || {
            font_spec(doc.styles().char(style), kind)
        });
        let unit = entry.space_width(shaper);
        let (ascent, descent) = entry.strut(shaper);
        let units: f32 = text[range.clone()]
            .chars()
            .map(|c| if c == '\t' { TAB_SPACES } else { 1.0 })
            .sum();
        Item {
            width: units * unit,
            ascent,
            descent,
            dy: 0.0,
            size: entry.spec().size.0 * scale,
            kind: ItemKind::Space { unit },
            range,
            brk,
            style,
        }
    }

    /// The items of `para`, in order.
    pub fn build(&mut self, para: &Paragraph) -> Vec<Item> {
        let text = para.text();
        let mut items: Vec<Item> = Vec::new();
        for seg in segment(para) {
            let item = match seg.kind {
                SegKind::Word => self.text_item(text, seg.range, seg.style, seg.brk),
                SegKind::Space => self.space_item(text, seg.range, seg.style, seg.brk),
                SegKind::Break => {
                    let (ascent, descent) = self.strut(seg.style);
                    marker(
                        seg.range,
                        ItemKind::Break,
                        seg.style,
                        Brk::Mandatory,
                        ascent,
                        descent,
                    )
                }
                SegKind::Object(id) => self.object_item(id, seg.range, seg.style, seg.brk),
            };
            if matches!(item.kind, ItemKind::Float { .. })
                && let Some(prev) = items.last_mut()
                && prev.brk == Brk::None
            {
                prev.brk = Brk::Allowed;
            }
            items.push(item);
        }
        items
    }

    fn object_item(
        &mut self,
        id: ObjectId,
        range: Range<usize>,
        style: CharStyleId,
        brk: Brk,
    ) -> Item {
        let Some(object) = self.doc.objects().get(id) else {
            return marker(range, ItemKind::Space { unit: 0.0 }, style, brk, 0.0, 0.0);
        };
        let size = (object.size.0.0 * self.scale, object.size.1.0 * self.scale);
        match object.wrap {
            Wrap::Inline => Item {
                width: size.0,
                ascent: size.1,
                descent: 0.0,
                dy: 0.0,
                size: size.1,
                kind: ItemKind::Object { id, height: size.1 },
                range,
                brk,
                style,
            },
            wrap => marker(
                range,
                ItemKind::Float { id, size, wrap },
                style,
                Brk::Allowed,
                0.0,
                0.0,
            ),
        }
    }
}

/// A zero-width item.
fn marker(
    range: Range<usize>,
    kind: ItemKind,
    style: CharStyleId,
    brk: Brk,
    ascent: f32,
    descent: f32,
) -> Item {
    Item {
        range,
        kind,
        width: 0.0,
        ascent,
        descent,
        dy: 0.0,
        brk,
        style,
        size: 0.0,
    }
}
