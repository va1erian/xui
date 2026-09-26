#![forbid(unsafe_code)]

//! The flat row list the tree paints: [`FlatNode`], the [`State`] that owns it,
//! the ancestor-expansion visibility filter and the indent/chevron geometry.
//!
//! Both the painter and the event mapper work from this materialized list, so
//! they share one definition of where a row's chevron, checkbox, label and
//! indent guides sit.

use std::rc::Rc;

use super::icon::RowIcon;
use super::model::{CheckState, NodeId, TreeModel, TreeRow};
use crate::geometry::Rect;
use crate::units::Dip;

/// The design height of a row.
pub(crate) const ROW: Dip = Dip(22.0);
/// The design left padding before a root's chevron.
const PAD: Dip = Dip(4.0);
/// The design horizontal step added per depth level.
const INDENT: Dip = Dip(16.0);
/// The design width of the chevron slot, reserved for every row.
const CHEVRON: Dip = Dip(16.0);
/// The design side of a row's checkbox.
const CHECK: Dip = Dip(16.0);
/// The design gap between a checkbox and its label.
const CHECK_GAP: Dip = Dip(6.0);
/// The design side of a row's leading icon.
const ICON: Dip = Dip(16.0);
/// The design gap between a leading icon and its label.
const ICON_GAP: Dip = Dip(6.0);
/// The design size of a row's label text.
pub(crate) const TEXT: Dip = Dip(12.0);

/// One materialized row: a flat [`TreeRow`] or a loaded model node.
#[derive(Clone, Debug)]
pub(crate) struct FlatNode {
    pub(crate) id: NodeId,
    pub(crate) label: String,
    pub(crate) depth: u16,
    pub(crate) expandable: bool,
    pub(crate) expanded: bool,
    pub(crate) checked: CheckState,
    pub(crate) icon: Option<RowIcon>,
    /// Whether a model node's children have been read; always set for flat rows.
    loaded: bool,
}

impl FlatNode {
    fn from_row(id: NodeId, row: &TreeRow) -> FlatNode {
        FlatNode {
            id,
            label: row.label.clone(),
            depth: row.depth,
            expandable: row.expandable,
            expanded: row.expanded,
            checked: row.checked,
            icon: row.icon.clone(),
            loaded: true,
        }
    }

    fn from_node(id: NodeId, depth: u16, node: super::model::TreeNode) -> FlatNode {
        FlatNode {
            id,
            label: node.label,
            depth,
            expandable: node.has_children,
            expanded: false,
            checked: node.checked,
            icon: node.icon,
            loaded: !node.has_children,
        }
    }
}

/// Where the tree's rows come from.
pub(crate) enum Source {
    /// A fixed, pre-flattened list of [`TreeRow`]s.
    Flat,
    /// A virtual model whose branches load on first expansion.
    Model(Rc<dyn TreeModel>),
}

/// The materialized rows plus the source they were built from.
pub(crate) struct State {
    pub(crate) rows: Vec<FlatNode>,
    pub(crate) source: Source,
}

impl State {
    /// Builds state from a flat `&[TreeRow]`; ids are the row indices.
    pub(crate) fn flat(rows: &[TreeRow]) -> State {
        let rows = rows
            .iter()
            .enumerate()
            .map(|(index, row)| FlatNode::from_row(index, row))
            .collect();
        State {
            rows,
            source: Source::Flat,
        }
    }

    /// Builds state from a model, reading the roots up front.
    pub(crate) fn model(model: Rc<dyn TreeModel>) -> State {
        let children = model.children(None);
        let rows = children
            .into_iter()
            .map(|node| {
                let id = node.id;
                FlatNode::from_node(id, 0, node)
            })
            .collect();
        State {
            rows,
            source: Source::Model(model),
        }
    }

    /// The raw index of the row with `id`, whether or not it is visible.
    pub(crate) fn find(&self, id: NodeId) -> Option<usize> {
        self.rows.iter().position(|row| row.id == id)
    }

    /// Expands or collapses the row at `index`, loading a model node's children
    /// on its first expansion. Returns whether the state changed.
    pub(crate) fn set_expanded(&mut self, index: usize, expanded: bool) -> bool {
        if !self.rows[index].expandable || self.rows[index].expanded == expanded {
            return false;
        }
        if expanded && !self.rows[index].loaded {
            self.load_children(index);
        }
        self.rows[index].expanded = expanded;
        true
    }

    fn load_children(&mut self, index: usize) {
        let Source::Model(model) = &self.source else {
            return;
        };
        let id = self.rows[index].id;
        let depth = self.rows[index].depth;
        let children = model.children(Some(id));
        let nodes = children
            .into_iter()
            .map(|node| {
                let id = node.id;
                FlatNode::from_node(id, depth + 1, node)
            })
            .collect::<Vec<_>>();
        self.rows[index].loaded = true;
        self.rows.splice(index + 1..index + 1, nodes);
    }
}

fn px(dip: Dip, dpi: u32) -> i32 {
    dip.to_px(dpi).value()
}

/// The device-pixel left/right padding of a row.
pub(crate) fn pad(dpi: u32) -> i32 {
    px(PAD, dpi)
}

/// The device-pixel width of the chevron slot.
pub(crate) fn chevron_px(dpi: u32) -> i32 {
    px(CHEVRON, dpi)
}

/// The x where `depth`'s chevron slot starts.
pub(crate) fn level_x(dpi: u32, left: i32, depth: u16) -> i32 {
    left + px(PAD, dpi) + depth as i32 * px(INDENT, dpi)
}

/// The x where a row's checkbox starts, just past the chevron slot.
pub(crate) fn checkbox_x(dpi: u32, left: i32, depth: u16) -> i32 {
    level_x(dpi, left, depth) + px(CHEVRON, dpi)
}

/// The x where a row's content starts, just past the checkbox when shown.
pub(crate) fn content_x(dpi: u32, left: i32, depth: u16, checkboxes: bool) -> i32 {
    let start = checkbox_x(dpi, left, depth);
    if checkboxes {
        start + px(CHECK, dpi) + px(CHECK_GAP, dpi)
    } else {
        start
    }
}

/// The x where a row's leading icon starts, just past the checkbox.
pub(crate) fn icon_x(dpi: u32, left: i32, depth: u16, checkboxes: bool) -> i32 {
    content_x(dpi, left, depth, checkboxes)
}

/// The x where a row's label starts, reserving the icon slot when shown.
pub(crate) fn label_x(dpi: u32, left: i32, depth: u16, checkboxes: bool, icon: bool) -> i32 {
    let start = content_x(dpi, left, depth, checkboxes);
    if icon {
        start + px(ICON, dpi) + px(ICON_GAP, dpi)
    } else {
        start
    }
}

/// The checkbox square of the row at `depth` whose top is `top`.
pub(crate) fn checkbox_rect(dpi: u32, left: i32, top: i32, depth: u16) -> Rect {
    let side = px(CHECK, dpi);
    let x = checkbox_x(dpi, left, depth);
    let y = top + (px(ROW, dpi) - side) / 2;
    Rect::new(x, y, x + side, y + side)
}

/// The icon box of the row at `depth` whose top is `top`.
pub(crate) fn icon_rect(dpi: u32, left: i32, top: i32, depth: u16, checkboxes: bool) -> Rect {
    let side = px(ICON, dpi);
    let x = icon_x(dpi, left, depth, checkboxes);
    let y = top + (px(ROW, dpi) - side) / 2;
    Rect::new(x, y, x + side, y + side)
}

/// Whether `x` hits the chevron of an expandable row at `depth`.
pub(crate) fn chevron_hit(dpi: u32, left: i32, depth: u16, expandable: bool, x: i32) -> bool {
    expandable && x >= level_x(dpi, left, depth) && x < checkbox_x(dpi, left, depth)
}

/// The x of the indent guide for a row at depth `level`.
pub(crate) fn guide_x(dpi: u32, left: i32, level: u16) -> i32 {
    level_x(dpi, left, level) + px(CHEVRON, dpi) / 2
}

/// The raw index of the row drawn at `y`, before the visibility filter.
pub(crate) fn row_at(dpi: u32, y: i32, count: usize) -> Option<usize> {
    if y < 0 {
        return None;
    }
    let index = (y / px(ROW, dpi).max(1)) as usize;
    (index < count).then_some(index)
}

/// Whether the row at `index` is shown: every ancestor row (the nearest
/// preceding row at each smaller depth) must be expanded.
pub(crate) fn is_visible(rows: &[FlatNode], index: usize) -> bool {
    let mut depth = rows[index].depth;
    let mut i = index;
    while depth > 0 {
        let parent_depth = depth - 1;
        let mut parent = None;
        while i > 0 {
            i -= 1;
            if rows[i].depth == parent_depth {
                parent = Some(i);
                break;
            }
            if rows[i].depth < parent_depth {
                break;
            }
        }
        match parent {
            Some(p) if rows[p].expandable && rows[p].expanded => depth = parent_depth,
            _ => return false,
        }
    }
    true
}

/// The visible slot the raw row at `index` is drawn at, or `None` when it is
/// hidden by a collapsed ancestor.
pub(crate) fn index_to_slot(rows: &[FlatNode], index: usize) -> Option<usize> {
    if index >= rows.len() || !is_visible(rows, index) {
        return None;
    }
    Some((0..index).filter(|at| is_visible(rows, *at)).count())
}

/// The raw row index drawn at visible slot `slot`, or `None` past the end.
pub(crate) fn slot_to_index(rows: &[FlatNode], slot: usize) -> Option<usize> {
    let mut visible = 0;
    for index in 0..rows.len() {
        if !is_visible(rows, index) {
            continue;
        }
        if visible == slot {
            return Some(index);
        }
        visible += 1;
    }
    None
}

/// Whether the ancestor at depth `level` of the row at `index` still has a
/// following sibling, so its indent guide continues through this row.
///
/// The nearest later visible row no deeper than `level` settles it: at exactly
/// `level` the ancestor has a sibling and the guide runs on; shallower, the
/// ancestor was the last child and the guide stops.
pub(crate) fn guide_continues(rows: &[FlatNode], index: usize, level: u16) -> bool {
    let mut i = index + 1;
    while i < rows.len() {
        if is_visible(rows, i) && rows[i].depth <= level {
            return rows[i].depth == level;
        }
        i += 1;
    }
    false
}
