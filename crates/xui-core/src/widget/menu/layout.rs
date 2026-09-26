#![forbid(unsafe_code)]

//! The [`Menu`](super::Menu) design values and its pure layout arithmetic.
//!
//! A bar tiles its titles horizontally; a popup stacks one entry per row. Every
//! function takes the origin (and, for popups, the width) the caller draws or
//! hit-tests in, so the same numbers serve both the painter and the event
//! mapper.

use crate::geometry::{Point, Rect};
use crate::units::Dip;

use super::model::{Kind, Node};

/// The design size of an entry's label.
pub(super) const TEXT_SIZE: Dip = Dip(12.0);
/// The design height of a popup entry.
pub(super) const ROW: Dip = Dip(24.0);
/// The design height of a divider.
pub(super) const SEPARATOR_ROW: Dip = Dip(7.0);
/// The vertical inset of the first and last popup entry.
pub(super) const PAD: Dip = Dip(5.0);
/// The design width of the check/radio mark column.
pub(super) const MARK: Dip = Dip(22.0);
/// The design width of the submenu arrow column.
pub(super) const ARROW: Dip = Dip(18.0);
/// The smallest popup width.
pub(super) const MIN_WIDTH: Dip = Dip(140.0);
/// The estimated width of one text character.
///
/// The portable [`Canvas`](crate::Canvas) does not measure text, so a label's
/// width is estimated from its character count; the row is still hit-tested by
/// its real rectangle.
pub(super) const CHAR: Dip = Dip(7.0);
/// The horizontal inset of a bar title's label.
pub(super) const BAR_PAD: Dip = Dip(12.0);
/// The design size of a check or radio mark.
pub(super) const BOX: Dip = Dip(16.0);
/// The design size of the submenu arrow glyph.
pub(super) const GLYPH: Dip = Dip(4.0);

/// The number of popups a context menu over `nodes` needs.
pub(super) fn level_count(nodes: &[Node]) -> usize {
    if nodes.is_empty() {
        return 0;
    }
    1 + nodes
        .iter()
        .filter(|node| node.kind == Kind::Submenu)
        .map(|node| level_count(&node.children))
        .max()
        .unwrap_or(0)
}

/// The number of popups a bar over `nodes` needs: titles live in the bar, so
/// only their children start the popup stack.
pub(super) fn bar_depth(nodes: &[Node]) -> usize {
    nodes
        .iter()
        .filter(|node| node.kind == Kind::Submenu)
        .map(|node| level_count(&node.children))
        .max()
        .unwrap_or(0)
}

/// The height of one entry in device pixels.
fn row_height(node: &Node, dpi: u32) -> i32 {
    match node.kind {
        Kind::Separator => SEPARATOR_ROW.to_px(dpi).value(),
        _ => ROW.to_px(dpi).value().max(1),
    }
}

/// The size a popup showing `nodes` needs.
pub(super) fn measure(nodes: &[Node], dpi: u32) -> (i32, i32) {
    let pad = PAD.to_px(dpi).value();
    let mark = MARK.to_px(dpi).value();
    let arrow = ARROW.to_px(dpi).value();
    let char_width = CHAR.to_px(dpi).value().max(1);
    let mut widest = 0;
    let mut height = pad * 2;
    for node in nodes {
        height += row_height(node, dpi);
        if node.kind == Kind::Separator {
            continue;
        }
        let extra = if node.kind == Kind::Submenu { arrow } else { 0 };
        widest = widest.max(mark + node.text.chars().count() as i32 * char_width + extra);
    }
    ((widest + pad * 2).max(MIN_WIDTH.to_px(dpi).value()), height)
}

/// Visits each popup entry with its row rectangle.
pub(super) fn each_row(
    nodes: &[Node],
    origin: Point,
    width: i32,
    dpi: u32,
    mut visit: impl FnMut(usize, Rect),
) {
    let mut top = origin.y + PAD.to_px(dpi).value();
    for (index, node) in nodes.iter().enumerate() {
        let height = row_height(node, dpi);
        visit(
            index,
            Rect::new(origin.x, top, origin.x + width, top + height),
        );
        top += height;
    }
}

/// The row rectangle of entry `index`.
pub(super) fn row_rect(
    nodes: &[Node],
    origin: Point,
    width: i32,
    dpi: u32,
    index: usize,
) -> Option<Rect> {
    let mut found = None;
    each_row(nodes, origin, width, dpi, |current, rect| {
        if current == index {
            found = Some(rect);
        }
    });
    found
}

/// The entry whose row contains the popup-local `y`, if any.
pub(super) fn row_at(nodes: &[Node], dpi: u32, y: i32) -> Option<usize> {
    let mut top = PAD.to_px(dpi).value();
    for (index, node) in nodes.iter().enumerate() {
        let height = row_height(node, dpi);
        if y >= top && y < top + height {
            return Some(index);
        }
        top += height;
    }
    None
}

/// The width of a bar title in device pixels.
fn title_width(node: &Node, dpi: u32) -> i32 {
    let char_width = CHAR.to_px(dpi).value().max(1);
    BAR_PAD.to_px(dpi).value() * 2 + node.text.chars().count() as i32 * char_width
}

/// Visits each bar title with its cell rectangle.
pub(super) fn each_title(
    nodes: &[Node],
    origin: Point,
    height: i32,
    dpi: u32,
    mut visit: impl FnMut(usize, Rect),
) {
    let mut left = origin.x;
    for (index, node) in nodes.iter().enumerate() {
        let width = title_width(node, dpi);
        visit(
            index,
            Rect::new(left, origin.y, left + width, origin.y + height),
        );
        left += width;
    }
}

/// The cell rectangle of bar title `index`.
pub(super) fn title_rect(
    nodes: &[Node],
    origin: Point,
    height: i32,
    dpi: u32,
    index: usize,
) -> Option<Rect> {
    let mut found = None;
    each_title(nodes, origin, height, dpi, |current, rect| {
        if current == index {
            found = Some(rect);
        }
    });
    found
}

/// The bar title whose cell contains the node-local `x`, if any.
pub(super) fn title_at(nodes: &[Node], x: i32, dpi: u32) -> Option<usize> {
    let mut left = 0;
    for (index, node) in nodes.iter().enumerate() {
        let width = title_width(node, dpi);
        if x >= left && x < left + width {
            return Some(index);
        }
        left += width;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widget::menu::MenuId;

    fn tree() -> Vec<Node> {
        vec![
            Node::command(MenuId::new(1), "&Open"),
            Node::separator(),
            Node::submenu(MenuId::new(2), "&Recent"),
            Node::check(MenuId::new(3), "Auto", false),
        ]
    }

    #[test]
    fn a_popup_measures_every_row() {
        let (width, height) = measure(&tree(), 96);
        assert!(width >= MIN_WIDTH.to_px(96).value());
        let row = ROW.to_px(96).value();
        let separator = SEPARATOR_ROW.to_px(96).value();
        assert_eq!(height, PAD.to_px(96).value() * 2 + row * 3 + separator);
    }

    #[test]
    fn hit_testing_maps_rows_and_titles() {
        let nodes = tree();
        let row = ROW.to_px(96).value();
        let pad = PAD.to_px(96).value();
        assert_eq!(row_at(&nodes, 96, pad + 1), Some(0));
        assert_eq!(row_at(&nodes, 96, pad + row + 1), Some(1));
        assert!(row_rect(&nodes, Point::new(0, 0), 140, 96, 2).is_some());

        let titles = vec![
            Node::submenu(MenuId::new(1), "File"),
            Node::submenu(MenuId::new(2), "Edit"),
        ];
        assert_eq!(title_at(&titles, 1, 96), Some(0));
        assert_eq!(title_at(&titles, 60, 96), Some(1));
        assert_eq!(title_at(&titles, 1000, 96), None);
    }

    #[test]
    fn the_popup_pool_grows_with_nesting() {
        let mut file = Node::submenu(MenuId::new(1), "File");
        file.children.push(Node::submenu(MenuId::new(2), "Recent"));
        file.children[0]
            .children
            .push(Node::command(MenuId::new(3), "a.txt"));
        let nodes = vec![file];
        assert_eq!(level_count(&nodes), 3, "the context root plus two submenus");
        assert_eq!(bar_depth(&nodes), 2, "a bar starts at the title's children");
        assert_eq!(level_count(&[]), 0);
        assert_eq!(bar_depth(&[]), 0);
    }
}
