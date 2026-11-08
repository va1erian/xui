#![forbid(unsafe_code)]

//! One block laid out: its indent, list marker, quote bars and text.

use super::cache::Shaped;
use super::flow::{Flow, flow};
use super::geom::{BlockLayout, Ctx, Decor, FRect, Piece};
use super::table::{self, Tables};
use crate::model::{Align, Block, BlockKind, Inline, Marker};
use crate::style::Role;

/// Spaces a tab in a code block stands for.
const TAB: &str = "    ";

/// Where a block sits among its neighbours.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Context {
    /// The space above it.
    pub gap: f32,
    /// How many quote levels it shares with the block above, whose bars run
    /// on through the gap.
    pub joined_quotes: u8,
}

/// Lays out block `index` in `width` device pixels.
pub(crate) fn layout(
    ctx: &mut Ctx,
    index: usize,
    width: f32,
    around: Context,
    tables: &mut Tables,
) -> BlockLayout {
    let block = &ctx.doc.blocks[index];
    let mut out = BlockLayout::default();
    let quote_step = ctx.px(ctx.style.quote_indent);
    let left =
        f32::from(block.quote) * quote_step + f32::from(block.list) * ctx.px(ctx.style.list_indent);
    let inner = (width - left).max(ctx.scale());
    let height = match &block.kind {
        BlockKind::Heading(level) => heading(ctx, block, *level, left, inner, &mut out),
        BlockKind::Paragraph => text(ctx, block, left, inner, &mut out).height,
        BlockKind::ListItem(marker) => {
            let flowed = text(ctx, block, left, inner, &mut out);
            self::marker(ctx, *marker, block.list, left, flowed.baseline, &mut out);
            flowed.height
        }
        BlockKind::Code { .. } => code(ctx, block, left, inner, &mut out),
        BlockKind::TableRow { .. } => table::row(ctx, index, left, inner, tables, &mut out),
        BlockKind::Rule => {
            let thin = (2.0 * ctx.scale()).round();
            out.decor.push(Decor::Fill(
                FRect::new(left, 0.0, inner, thin),
                Role::Border,
            ));
            thin
        }
    };
    let bar = (3.0 * ctx.scale()).round();
    for level in 0..block.quote {
        let top = if level < around.joined_quotes {
            -around.gap
        } else {
            0.0
        };
        let x = f32::from(level) * quote_step;
        out.decor.push(Decor::Fill(
            FRect::new(x, top, bar, height - top),
            Role::Border,
        ));
    }
    out.height = height;
    out
}

fn base_flow<'s>(ctx: &Ctx<'s>, block: &Block, heading: Option<u8>) -> Flow<'s> {
    Flow {
        spans: ctx.doc.spans(block),
        heading,
        extra: Inline::NONE,
        quoted: block.quote > 0,
        align: Align::None,
    }
}

fn text(
    ctx: &mut Ctx,
    block: &Block,
    left: f32,
    width: f32,
    out: &mut BlockLayout,
) -> super::flow::Flowed {
    let f = base_flow(ctx, block, None);
    flow(ctx, &f, (left, 0.0), width, out)
}

/// A heading; levels 1 and 2 are underlined by a rule.
fn heading(
    ctx: &mut Ctx,
    block: &Block,
    level: u8,
    left: f32,
    width: f32,
    out: &mut BlockLayout,
) -> f32 {
    let f = base_flow(ctx, block, Some(level));
    let height = flow(ctx, &f, (left, 0.0), width, out).height;
    if level > 2 {
        return height;
    }
    let pad = (4.0 * ctx.scale()).round();
    let thin = ctx.scale().round().max(1.0);
    out.decor.push(Decor::Fill(
        FRect::new(left, height + pad, width, thin),
        Role::Border,
    ));
    height + pad + thin
}

/// The marker of a list item, in the indent before `left`.
fn marker(
    ctx: &mut Ctx,
    marker: Marker,
    depth: u8,
    left: f32,
    baseline: f32,
    out: &mut BlockLayout,
) {
    let em = ctx.px(ctx.style.size);
    let gap = 0.45 * em;
    match marker {
        Marker::Bullet => {
            let d = (0.36 * em).round();
            let rect = FRect::new(left - gap - d, baseline - 0.33 * em - d / 2.0, d, d);
            // Nested levels alternate a disc with a square, as browsers do.
            out.decor.push(if depth % 2 == 1 {
                Decor::Disc(rect, Role::Text)
            } else {
                Decor::Fill(rect, Role::Text)
            });
        }
        Marker::Task(checked) => {
            let d = (0.8 * em).round();
            let rect = FRect::new(left - gap - d, baseline - 0.75 * em, d, d);
            out.decor.push(Decor::Frame(rect, Role::Weak));
            if checked {
                let inset = (0.2 * em).round();
                let tick = FRect::new(
                    rect.x + inset,
                    rect.y + inset,
                    d - 2.0 * inset,
                    d - 2.0 * inset,
                );
                out.decor.push(Decor::Fill(tick, Role::Link));
            }
        }
        Marker::Number(n) => {
            let label = format!("{n}.");
            let font = ctx.style.font(Inline::NONE, None, false);
            let shaped = ctx.cache.word(ctx.shaper, ctx.style, font, &label, ctx.dpi);
            out.pieces.push(Piece {
                x: left - gap - shaped.width,
                y: baseline - shaped.baseline,
                layout: shaped.layout,
                role: Role::Text,
            });
        }
    }
}

/// A code block: unwrapped lines on a shaded box, clipped to it.
fn code(ctx: &mut Ctx, block: &Block, left: f32, width: f32, out: &mut BlockLayout) -> f32 {
    let inset = ctx.px(ctx.style.inset);
    let font = ctx.style.font(Inline::NONE, None, true);
    let spec = ctx.style.spec(font);
    let line_height = ctx
        .cache
        .metrics(ctx.shaper, ctx.style, font, ctx.dpi)
        .height;
    let mut y = inset;
    let mut at = block.text.start;
    for line in ctx.doc.block_text(block).split('\n') {
        let end = at + line.len() as u32;
        if !line.trim_end().is_empty() {
            let shaped = if line.contains('\t') {
                Shaped::new(ctx.shaper.layout(
                    &line.replace('\t', TAB),
                    &spec,
                    f32::INFINITY,
                    ctx.dpi,
                ))
            } else {
                Shaped::new(ctx.shaper.layout(line, &spec, f32::INFINITY, ctx.dpi))
            };
            out.pieces.push(Piece {
                layout: shaped.layout,
                x: left + inset,
                y,
                role: if block.quote > 0 {
                    Role::Weak
                } else {
                    Role::Text
                },
            });
        }
        y += line_height;
        at = end + 1;
    }
    let height = y + inset;
    out.decor.insert(
        0,
        Decor::Fill(FRect::new(left, 0.0, width, height), Role::Surface),
    );
    out.clip = Some(FRect::new(
        left + inset,
        0.0,
        (width - 2.0 * inset).max(0.0),
        height,
    ));
    height
}
