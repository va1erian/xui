#![forbid(unsafe_code)]

//! Turning model styles into layout numbers: effective fonts, the defaults of
//! a [`BlockKind`], and the indents of lists and quotes, all in device pixels.

use xui_core::Dip;
use xui_core::backend::FontSpec;

use crate::model::{Baseline, BlockKind, CharStyle, LineSpacing, ParaStyle};

/// Each nesting level of a list indents by this much.
const LIST_INDENT: Dip = Dip(24.0);
/// The gap between a list marker and its text.
const MARKER_GAP: Dip = Dip(6.0);
/// A quote is indented by this much, with its rule inside the indent.
const QUOTE_INDENT: Dip = Dip(24.0);
/// Super- and subscripts are set at this fraction of the size.
const SCRIPT_SCALE: f32 = 0.65;

/// Pixels per `Dip` at `dpi`.
pub(crate) fn scale_for(dpi: u32) -> f32 {
    dpi as f32 / 96.0
}

/// The font a run of `style` is shaped with inside a paragraph of `kind`:
/// headings are larger and bold, scripts are smaller.
pub(crate) fn font_spec(style: &CharStyle, kind: BlockKind) -> FontSpec {
    let mut size = style.size.0;
    let mut weight = style.weight.value();
    if let BlockKind::Heading(level) = kind {
        size *= match level {
            1 => 1.8,
            2 => 1.4,
            _ => 1.2,
        };
        weight = weight.max(700);
    }
    if style.baseline != Baseline::Normal {
        size *= SCRIPT_SCALE;
    }
    let mut spec = FontSpec::new(Dip(size)).weight(weight).italic(style.italic);
    spec.family = style.family.clone();
    spec
}

/// How far a run sits below the baseline (negative is raised), in pixels,
/// given the [`font_spec`] size of the run.
pub(crate) fn baseline_shift(style: &CharStyle, spec_size: Dip, scale: f32) -> f32 {
    let em = spec_size.0 / SCRIPT_SCALE * scale;
    match style.baseline {
        Baseline::Normal => 0.0,
        Baseline::Superscript => -0.35 * em,
        Baseline::Subscript => 0.15 * em,
    }
}

/// A paragraph's metrics in pixels.
pub(crate) struct ParaMetrics {
    pub left: f32,
    pub right: f32,
    pub first: f32,
    pub before: f32,
    pub after: f32,
    pub spacing: Spacing,
    /// Where a list marker ends (its gap from the text), if a list item.
    pub marker_gap: f32,
}

/// Line spacing in pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Spacing {
    Multiple(f32),
    Exactly(f32),
}

/// Resolves `style` at `scale`.
pub(crate) fn para_metrics(style: &ParaStyle, scale: f32) -> ParaMetrics {
    let px = |d: Dip| d.0 * scale;
    let mut left = px(style.indent_left);
    let (mut before, mut after) = (px(style.space_before), px(style.space_after));
    if let Some(item) = style.list {
        left += px(LIST_INDENT) * (f32::from(item.level) + 1.0);
    }
    match style.kind {
        BlockKind::Body => {}
        BlockKind::Heading(level) => {
            let (b, a) = match level {
                1 => (14.0, 6.0),
                2 => (12.0, 5.0),
                _ => (10.0, 4.0),
            };
            before += b * scale;
            after += a * scale;
        }
        BlockKind::Quote => {
            left += px(QUOTE_INDENT);
            before += 4.0 * scale;
            after += 4.0 * scale;
        }
    }
    ParaMetrics {
        left,
        right: px(style.indent_right),
        first: px(style.indent_first),
        before,
        after,
        spacing: match style.line_spacing {
            LineSpacing::Multiple(m) => Spacing::Multiple(m.max(0.1)),
            LineSpacing::Exactly(d) => Spacing::Exactly(px(d).max(1.0)),
        },
        marker_gap: px(MARKER_GAP),
    }
}

/// The x of the quote rule for a paragraph indented by `left`.
pub(crate) fn quote_rule_x(left: f32, scale: f32) -> f32 {
    (left - (QUOTE_INDENT.0 * 0.5) * scale).max(0.0)
}
