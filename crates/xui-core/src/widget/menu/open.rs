#![forbid(unsafe_code)]

//! Opening and closing the [`Menu`](super::Menu) popup stack.
//!
//! Popups are pooled, one node per nesting level, and shown/hidden through the
//! backend the way [`ComboBox`](super::ComboBox) shows its list.

use std::rc::Rc;

use super::{Level, Open, Runtime, layout, model};
use crate::backend::WidgetId;
use crate::geometry::{Point, Rect};

/// Opens the menu of bar title `index`.
pub(super) fn open_bar<M: 'static>(rt: &Rc<Runtime<M>>, index: usize) {
    close_levels_from(rt, 0);
    let title = {
        let model = rt.model.borrow();
        match model.get(index) {
            Some(node) if node.enabled && !node.children.is_empty() => {
                rt.bar.get().and_then(|bar| {
                    let bounds = rt.ui.bounds(bar);
                    layout::title_rect(
                        &model,
                        Point::new(bounds.left, bounds.top),
                        bounds.height(),
                        rt.ui.dpi(),
                        index,
                    )
                })
            }
            _ => None,
        }
    };
    let Some(rect) = title else {
        return;
    };
    rt.view.borrow_mut().bar_open = Some(index);
    let origin = Rect::new(rect.left, rect.bottom - 1, rect.left, rect.bottom - 1);
    open_level(rt, vec![index], origin);
    if let Some(bar) = rt.bar.get() {
        rt.ui.invalidate(bar);
    }
}

/// Shows a context popup at the client point `at`.
pub(super) fn show_context<M: 'static>(rt: &Rc<Runtime<M>>, at: Point) {
    close_all(rt);
    if rt.model.borrow().is_empty() {
        return;
    }
    open_level(rt, Vec::new(), Rect::new(at.x, at.y, at.x, at.y));
}

/// Opens a popup for the entry list at `path`, anchored at `origin`.
fn open_level<M: 'static>(rt: &Rc<Runtime<M>>, path: Vec<usize>, origin: Rect) {
    let depth = rt.view.borrow().levels.len();
    let Some(id) = rt.pool.borrow().get(depth).copied() else {
        return;
    };
    let Some((first, (width, height))) =
        model::with_entries(&rt.model.borrow(), &path, |entries| {
            (
                model::first_selectable(entries),
                layout::measure(entries, rt.ui.dpi()),
            )
        })
    else {
        return;
    };
    let client = rt.ui.client_rect();
    let left = origin
        .left
        .min((client.right - width).max(client.left))
        .max(client.left);
    let top = origin
        .top
        .min((client.bottom - height).max(client.top))
        .max(client.top);
    let bounds = Rect::new(left, top, left + width, top + height);
    rt.ui.apply_moves(&[(id, bounds)]);
    rt.ui.set_visible(id, true);
    rt.ui.raise(id);
    rt.ui.focus(id);
    rt.ui.invalidate(id);
    rt.view
        .borrow_mut()
        .levels
        .push(Level { path, hover: first });
    rt.open.borrow_mut().push(Open { id, bounds });
}

/// Opens the submenu of entry `index` at `depth`.
pub(super) fn open_submenu<M: 'static>(rt: &Rc<Runtime<M>>, depth: usize, index: usize) {
    let Some(path) = level_path(rt, depth) else {
        return;
    };
    let mut child = path.clone();
    child.push(index);
    if rt
        .view
        .borrow()
        .levels
        .get(depth + 1)
        .is_some_and(|level| level.path == child)
    {
        return;
    }
    close_levels_from(rt, depth + 1);
    let Some(parent) = rt.open.borrow().get(depth).map(|open| open.bounds) else {
        return;
    };
    let row = {
        let model = rt.model.borrow();
        model::with_entries(&model, &path, |entries| {
            layout::row_rect(
                entries,
                Point::new(parent.left, parent.top),
                parent.width(),
                rt.ui.dpi(),
                index,
            )
        })
        .flatten()
    };
    let Some(row) = row else {
        return;
    };
    open_level(rt, child, Rect::new(row.right, row.top, row.right, row.top));
}

/// Hides every popup from `depth` outward.
pub(super) fn close_levels_from<M: 'static>(rt: &Runtime<M>, depth: usize) {
    let ids: Vec<WidgetId> = {
        let open = rt.open.borrow();
        if depth >= open.len() {
            return;
        }
        open[depth..].iter().map(|entry| entry.id).collect()
    };
    for id in ids {
        rt.ui.set_visible(id, false);
    }
    rt.open.borrow_mut().truncate(depth);
    rt.view.borrow_mut().levels.truncate(depth);
}

/// Hides every popup and clears the bar's open highlight.
pub(super) fn close_all<M: 'static>(rt: &Runtime<M>) {
    close_levels_from(rt, 0);
    if rt.view.borrow_mut().bar_open.take().is_some()
        && let Some(bar) = rt.bar.get()
    {
        rt.ui.invalidate(bar);
    }
}

/// Repaints the bar and every open popup, after the model changed.
pub(super) fn redraw<M: 'static>(rt: &Runtime<M>) {
    if let Some(bar) = rt.bar.get() {
        rt.ui.invalidate(bar);
    }
    for entry in rt.open.borrow().iter() {
        rt.ui.invalidate(entry.id);
    }
}

/// Focuses the popup open at `depth` and repaints it.
pub(super) fn focus_level<M: 'static>(rt: &Runtime<M>, depth: usize) {
    if let Some(id) = rt.pool.borrow().get(depth).copied() {
        rt.ui.focus(id);
        rt.ui.invalidate(id);
    }
}

/// Repaints the popup open at `depth`.
pub(super) fn invalidate_level<M: 'static>(rt: &Runtime<M>, depth: usize) {
    if let Some(id) = rt.pool.borrow().get(depth).copied() {
        rt.ui.invalidate(id);
    }
}

/// The index path of the entry list shown at `depth`.
pub(super) fn level_path<M: 'static>(rt: &Runtime<M>, depth: usize) -> Option<Vec<usize>> {
    rt.view
        .borrow()
        .levels
        .get(depth)
        .map(|level| level.path.clone())
}
