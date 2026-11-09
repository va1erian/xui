#![forbid(unsafe_code)]

//! The document's images: which the app has supplied, which it has been asked
//! for, and the blocks to lay out again when one arrives.
//!
//! The layout loads nothing. Laying out a block notes the images in it that
//! have no pixels yet; their sources wait in a queue the view drains into the
//! app's `on_image` messages, each source once per document.

use std::collections::HashSet;
use std::sync::Arc;

use xui_core::image::Image;

use super::Layout;
use super::flow::Flow;
use super::geom::Ctx;
use crate::model::{BlockKind, Document};

/// The image state of one document.
#[derive(Default)]
pub(super) struct Images {
    /// Per image of the document: its pixels, once the app gave them.
    pixels: Vec<Option<Arc<Image>>>,
    /// Per image: the block holding it, if any block shows it.
    blocks: Vec<Option<u32>>,
    /// Sources already reported.
    asked: HashSet<Box<str>>,
    /// Sources to report, in the order their blocks were laid out.
    requests: Vec<Box<str>>,
}

impl Images {
    pub fn new(doc: &Document) -> Images {
        let mut blocks = vec![None; doc.images().len()];
        for (index, block) in doc.blocks().iter().enumerate() {
            for span in doc.spans(block) {
                if let Some(slot) = span.image.and_then(|i| blocks.get_mut(i as usize)) {
                    *slot = Some(index as u32);
                }
            }
        }
        Images {
            pixels: vec![None; doc.images().len()],
            blocks,
            ..Images::default()
        }
    }

    /// Per image: its pixels, if the app gave them.
    pub fn pixels(&self) -> &[Option<Arc<Image>>] {
        &self.pixels
    }

    /// Queues the sources of `wanted` images not yet asked for.
    pub fn want(&mut self, doc: &Document, wanted: &[u32]) {
        for &index in wanted {
            let src = &doc.image(index).src;
            if self.pixels[index as usize].is_none() && !self.asked.contains(src) {
                self.asked.insert(src.clone());
                self.requests.push(src.clone());
            }
        }
    }
}

impl Layout {
    /// Shows `image` for every image of the document whose source is `src`,
    /// laying their blocks out again. Returns whether the document has one.
    pub fn set_image(&mut self, src: &str, image: &Arc<Image>) -> bool {
        let mut found = false;
        for index in 0..self.images.pixels.len() {
            if &*self.doc.images()[index].src != src {
                continue;
            }
            found = true;
            self.images.pixels[index] = Some(Arc::clone(image));
            let Some(block) = self.images.blocks[index].map(|b| b as usize) else {
                continue;
            };
            // A table's columns are sized from all its rows.
            let blocks = match self.doc.blocks().get(block).map(|b| &b.kind) {
                Some(BlockKind::TableRow { table, .. }) => self.tables.forget(*table as usize),
                _ => block..block + 1,
            };
            for block in blocks {
                self.forget(block);
            }
        }
        found
    }

    /// Drops block `index`'s layout. `work` lays it out again, or `ensure`
    /// if it comes into view first; its old height stands until then.
    fn forget(&mut self, index: usize) {
        if let Some(slot) = self.slots.get_mut(index)
            && slot.layout.take().is_some()
        {
            slot.made = 0;
            self.pending = self.pending.min(index);
            self.generation += 1;
        }
    }

    /// The pixels of image `index`, if the app gave them.
    pub fn image(&self, index: u32) -> Option<&Arc<Image>> {
        self.images.pixels.get(index as usize)?.as_ref()
    }

    /// The sources laid-out blocks need that the app has not been asked for,
    /// each once per document.
    pub fn take_requests(&mut self) -> Vec<Box<str>> {
        std::mem::take(&mut self.images.requests)
    }
}

/// An image's place on a line, in device pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Picture {
    pub image: u32,
    pub w: f32,
    pub h: f32,
}

/// Image `image`'s size on a line `width` wide, when the app has supplied
/// it: an image pixel per design unit, scaled down to fit the line.
pub(super) fn picture(ctx: &Ctx, image: u32, width: f32) -> Option<Picture> {
    let pixels = ctx.images.get(image as usize)?.as_ref()?;
    let (w, h) = (
        pixels.width() as f32 * ctx.scale(),
        pixels.height() as f32 * ctx.scale(),
    );
    let fit = if width.is_finite() && w > width {
        width / w
    } else {
        1.0
    };
    Some(Picture {
        image,
        w: (w * fit).round().max(1.0),
        h: (h * fit).round().max(1.0),
    })
}

/// Whether `pos` falls strictly inside the text of an image that has its
/// pixels, where a line must not break.
pub(super) fn inside_image(ctx: &Ctx, flow: &Flow, pos: u32) -> bool {
    let i = flow.spans.partition_point(|s| s.range.end <= pos);
    let Some(image) = flow.spans.get(i).and_then(|s| s.image) else {
        return false;
    };
    let loaded = ctx.images.get(image as usize).is_some_and(Option::is_some);
    let started = pos > flow.spans[i].range.start
        || i.checked_sub(1)
            .is_some_and(|p| flow.spans[p].image == Some(image));
    loaded && started
}
