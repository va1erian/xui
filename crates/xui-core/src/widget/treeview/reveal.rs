#![forbid(unsafe_code)]

//! The expand/collapse animation's state and paint maths.
//!
//! The model flips `expanded` at once; this only remembers which rows the
//! flip showed or hid, so the painter can keep drawing them, clipped to a band
//! under the node, until the animation lands.

use std::time::Duration;

use super::flatten::State;
use super::model::NodeId;
use crate::motion::Animated;

/// The node being animated and the rows it showed or hid.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Reveal {
    node: NodeId,
    /// The raw index of the first descendant; the node itself is just before.
    first: usize,
    /// One past the raw index of the last descendant.
    end: usize,
    /// How many rows the flip showed or hid.
    rows: usize,
}

/// Records that the node at `index` was just flipped to `expanded` and
/// returns the value the animation must move to. `before` is the visible row
/// count from before the flip.
///
/// A flip that moved no row has nothing to animate and returns `None`. An
/// animation of another node simply ends: the new one replaces it.
pub(crate) fn begin(
    state: &mut State,
    value: &Animated,
    index: usize,
    expanded: bool,
    before: usize,
) -> Option<f32> {
    let rows = state.visible_len().abs_diff(before);
    if rows == 0 {
        state.reveal = None;
        return None;
    }
    let depth = state.rows[index].depth;
    let end = (index + 1..state.rows.len())
        .find(|at| state.rows[*at].depth <= depth)
        .unwrap_or(state.rows.len());
    let node = state.rows[index].id;
    if state.reveal.is_none_or(|reveal| reveal.node != node) {
        value.jump(if expanded { 0.0 } else { 1.0 });
    }
    state.reveal = Some(Reveal {
        node,
        first: index + 1,
        end,
        rows,
    });
    Some(if expanded { 1.0 } else { 0.0 })
}

/// A reveal in flight, as one paint sees it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Active {
    /// The raw index of the animated node.
    pub(crate) node: usize,
    end: usize,
    progress: f32,
    rows: usize,
    capped: usize,
}

impl Active {
    /// A reveal at `progress` (0 closed, 1 open) of `rows` rows, `fit` of which
    /// fit the viewport: a huge subtree animates as a viewport's worth.
    pub(crate) fn new(node: usize, end: usize, progress: f32, rows: usize, fit: usize) -> Active {
        Active {
            node,
            end,
            progress: progress.clamp(0.0, 1.0),
            rows,
            capped: rows.min(fit),
        }
    }

    /// Whether the raw row `index` is one of the node's descendants.
    pub(crate) fn contains(&self, index: usize) -> bool {
        index > self.node && index < self.end
    }

    /// Whether the raw row `index` comes after the node's subtree.
    pub(crate) fn after(&self, index: usize) -> bool {
        index >= self.end
    }

    /// The open fraction, which also turns the node's chevron.
    pub(crate) fn amount(&self) -> f32 {
        self.progress
    }

    /// The height of the band under the node that shows its descendants.
    pub(crate) fn band_height(&self, row_height: i32) -> i32 {
        (self.progress * (self.capped as i32 * row_height) as f32).round() as i32
    }

    /// How far a row after the subtree sits above its final position. The
    /// first of them starts at the band's lower edge.
    pub(crate) fn shift(&self, row_height: i32) -> i32 {
        self.rows as i32 * row_height - self.band_height(row_height)
    }
}

/// The reveal to paint at `now`, if one is in flight and still describes the
/// rows: a model change that moved them ends it.
pub(crate) fn active(
    state: &State,
    value: &Animated,
    now: Duration,
    row_height: i32,
    height: i32,
) -> Option<Active> {
    let reveal = state.reveal?;
    if !value.is_running(now) {
        return None;
    }
    let node = reveal.first.checked_sub(1)?;
    if reveal.end > state.rows.len() || state.rows.get(node)?.id != reveal.node {
        return None;
    }
    let fit = (height.max(0) / row_height.max(1)) as usize + 1;
    Some(Active::new(
        node,
        reveal.end,
        value.value(now),
        reveal.rows,
        fit,
    ))
}
