#![forbid(unsafe_code)]

//! The text a mounted layout contributes to [`Ui::layout_report`]: its tree,
//! where each node landed, and what looks wrong.
//!
//! [`Ui::layout_report`]: crate::app::Ui::layout_report

use std::fmt::Write as _;

use super::mount::{Area, State};
use crate::geometry::Rect;
use crate::layout::{Constraints, TraceNode, Warning};

/// Appends `state`'s report to `out`.
pub(super) fn write<M: 'static>(state: &State<M>, out: &mut String) {
    let area = state.placed_in.get();
    let _ = match state.area {
        Area::Window => writeln!(out, "layout in the window, {}", rect(area)),
        Area::Node(id) => writeln!(out, "layout in node {}, {}", id.raw(), rect(area)),
        Area::Driven(id) => writeln!(out, "layout driven by node {}, {}", id.raw(), rect(area)),
    };
    if area.is_empty() {
        out.push_str("  (not placed: its area is empty)\n");
        return;
    }
    let dpi = state.ui.dpi();
    let trace = state.tree.trace(area, dpi, &|key, c| state.leaf(key, c));
    let name = |index: usize| match trace[index].node {
        TraceNode::Group(kind) => kind.name().to_string(),
        TraceNode::Leaf(key) => describe(state, key),
    };
    for (index, node) in trace.iter().enumerate() {
        let indent = "  ".repeat(node.depth + 1);
        let _ = write!(out, "{indent}{} {}", name(index), rect(node.rect));
        let mut warnings: Vec<String> = node
            .warnings
            .iter()
            .map(|warning| match warning {
                Warning::Outside => "outside its parent: clipped".to_string(),
                Warning::ZeroSize => "zero size".to_string(),
                Warning::Overlaps(other) => format!("overlaps {}", name(*other)),
            })
            .collect();
        if let TraceNode::Leaf(key) = node.node
            && let Some(needs) = truncated(state, key, node.rect)
        {
            warnings.push(format!("text truncated: needs {needs}px wide"));
        }
        for warning in warnings {
            let _ = write!(out, "  ! {warning}");
        }
        out.push('\n');
    }
}

/// Appends the rectangle of every node `state` placed, groups included.
pub(super) fn rects<M: 'static>(state: &State<M>, out: &mut Vec<Rect>) {
    let area = state.placed_in.get();
    if area.is_empty() {
        return;
    }
    let trace = state
        .tree
        .trace(area, state.ui.dpi(), &|key, c| state.leaf(key, c));
    out.extend(trace.iter().map(|node| node.rect));
}

/// A leaf's widget type and, for one with text, the text.
fn describe<M: 'static>(state: &State<M>, key: usize) -> String {
    let widget = &state.widgets[key];
    let full = widget.type_name();
    let base = full.split('<').next().unwrap_or(full);
    let short = base.rsplit("::").next().unwrap_or(base);
    match widget.layout_text() {
        Some(text) => format!("{short} {text:?}"),
        None => short.to_string(),
    }
}

/// The width a text widget needs when it is narrower than that at `rect`.
fn truncated<M: 'static>(state: &State<M>, key: usize, rect: Rect) -> Option<i32> {
    let widget = &state.widgets[key];
    widget.layout_text()?;
    let constraints = Constraints::unbounded(state.ui.dpi()).with_width(rect.width());
    let needs = widget.measure(&state.ui, constraints).width;
    (needs > rect.width()).then_some(needs)
}

fn rect(rect: Rect) -> String {
    format!(
        "at {},{} size {}x{}",
        rect.left,
        rect.top,
        rect.width(),
        rect.height()
    )
}
