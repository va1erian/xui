#![forbid(unsafe_code)]

//! The space between blocks, and a block's height guessed before it is laid
//! out.

use crate::model::{Block, BlockKind, Document};
use crate::style::MarkdownStyle;

/// The space above block `index`.
pub(super) fn gap(doc: &Document, index: usize, style: &MarkdownStyle, scale: f32) -> f32 {
    let Some(prev) = index.checked_sub(1).map(|i| &doc.blocks()[i]) else {
        return 0.0;
    };
    let block = &doc.blocks()[index];
    let gap = style.gap.0 * scale;
    match (&prev.kind, &block.kind) {
        (BlockKind::TableRow { table: a, .. }, BlockKind::TableRow { table: b, .. }) if a == b => {
            0.0
        }
        (_, BlockKind::Heading(_)) => gap * 1.6,
        (_, BlockKind::ListItem(_)) if prev.list > 0 => gap * 0.4,
        _ => gap,
    }
}

/// A block's height guessed from its length, before it is shaped.
pub(super) fn estimate(
    doc: &Document,
    block: &Block,
    style: &MarkdownStyle,
    scale: f32,
    width: f32,
) -> f32 {
    let em = style.size.0 * scale;
    let inset = style.inset.0 * scale;
    let text = doc.block_text(block);
    match &block.kind {
        BlockKind::Rule => 2.0 * scale,
        BlockKind::Code { .. } => {
            let size = style.code_size.0 * scale;
            // A wrapped line takes a row per box width of characters.
            let per_row = if style.wrap_code {
                ((width - 2.0 * inset) / (size * 0.6)).max(1.0)
            } else {
                f32::INFINITY
            };
            let rows: f32 = text
                .split('\n')
                .map(|line| (line.chars().count() as f32 / per_row).ceil().max(1.0))
                .sum();
            rows * size * 1.3 + 2.0 * inset
        }
        BlockKind::TableRow { .. } => em * 1.35 + 2.0 * inset,
        kind => {
            let size = match kind {
                BlockKind::Heading(level) => {
                    em * style.heading_scale[usize::from((*level).clamp(1, 6)) - 1]
                }
                _ => em,
            };
            let per_line = (width / (size * 0.5)).max(1.0);
            let lines = (text.chars().count() as f32 / per_line).ceil().max(1.0);
            lines * size * 1.35
        }
    }
}
