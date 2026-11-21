#![forbid(unsafe_code)]

//! Opening and closing the [`Menu`](super::Menu) popup stack.
//!
//! Popups are pooled, one node per nesting level, and shown/hidden through the
//! backend the way [`ComboBox`](super::ComboBox) shows its list.

use std::cell::Cell;
use std::rc::Rc;

use super::{Level, Open, Runtime, layout, model};
use crate::backend::WidgetId;
use crate::geometry::{Point, Rect};

/// Opens the menu of bar title `index`.
pub(super) fn open_bar<M: 'static>(rt: &Rc<Runtime<M>>, index: usize) {
    // Re-opening the title whose menu is already open would hide and re-show
    // its popup. The pointer moves over the bar fire continuously, so that
    // churn flickers the open drop-down; leave the stack untouched instead.
    if rt.view.borrow().bar_open == Some(index) {
        return;
    }
    // Moving along the bar swaps one open menu for the next at once; only a
    // menu opening from closed unrolls.
    let swapping = !rt.open.borrow().is_empty();
    if swapping {
        hide_levels_from(rt, 0);
    }
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
    open_level(rt, vec![index], origin, !swapping);
    if let Some(bar) = rt.bar.get() {
        rt.ui.invalidate(bar);
    }
}

/// Shows a context popup at the client point `at`.
pub(super) fn show_context<M: 'static>(rt: &Rc<Runtime<M>>, at: Point) {
    // Showing it again where it is already open moves it, without unrolling.
    let swapping = !rt.open.borrow().is_empty();
    if swapping {
        hide_all(rt);
    } else {
        close_all(rt);
    }
    if rt.model.borrow().is_empty() {
        return;
    }
    open_level(rt, Vec::new(), Rect::new(at.x, at.y, at.x, at.y), !swapping);
}

/// Opens a popup for the entry list at `path`, anchored at `origin`; it
/// unrolls when `animate` is set.
fn open_level<M: 'static>(rt: &Rc<Runtime<M>>, path: Vec<usize>, origin: Rect, animate: bool) {
    let depth = rt.view.borrow().levels.len();
    let Some(id) = rt.pool.borrow().get(depth).copied() else {
        return;
    };
    let Some(unroll) = rt.unrolls.borrow().get(depth).cloned() else {
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
    // Record the level before showing: a native backend paints a popup
    // synchronously inside `set_visible` (so its first composed frame is
    // finished), and the painter reads this state (#130). A popup of this
    // depth still rolling shut is reopened, so it no longer closes.
    {
        let mut view = rt.view.borrow_mut();
        view.closing.retain(|(at, _)| *at != depth);
        view.levels.push(Level { path, hover: first });
    }
    rt.open.borrow_mut().push(Open { id, bounds });
    unroll.open(&rt.ui, id, bounds, animate);
    rt.ui.raise(id);
    focus_node(rt, id);
    rt.ui.invalidate(id);
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
    // A submenu replacing its sibling swaps at once; one opening alone unrolls.
    let swapping = rt.view.borrow().levels.len() > depth + 1;
    if swapping {
        hide_levels_from(rt, depth + 1);
    }
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
    let origin = Rect::new(row.right, row.top, row.right, row.top);
    open_level(rt, child, origin, !swapping);
}

/// Closes every popup from `depth` outward. The one at `depth` rolls shut;
/// those nested in it hide at once, so a whole menu closing is one popup
/// shrinking. The popup's state outlives the truncation in the view's
/// `closing` list until it has landed.
pub(super) fn close_levels_from<M: 'static>(rt: &Runtime<M>, depth: usize) {
    let id = {
        let mut open = rt.open.borrow_mut();
        let Some(entry) = open.get(depth) else {
            return;
        };
        let id = entry.id;
        open.truncate(depth);
        id
    };
    let level = rt.view.borrow_mut().levels.drain(depth..).next();
    hide_pooled_from(rt, depth + 1);
    let Some(unroll) = rt.unrolls.borrow().get(depth).cloned() else {
        return;
    };
    {
        let mut view = rt.view.borrow_mut();
        view.closing.retain(|(at, _)| *at != depth);
        view.closing.extend(level.map(|level| (depth, level)));
    }
    let view = Rc::clone(&rt.view);
    unroll.close(&rt.ui, id, move || {
        view.borrow_mut().closing.retain(|(at, _)| *at != depth);
    });
}

/// Hides every popup from `depth` outward at once, dropping any that is
/// still rolling shut.
fn hide_levels_from<M: 'static>(rt: &Runtime<M>, depth: usize) {
    hide_pooled_from(rt, depth);
    rt.open.borrow_mut().truncate(depth);
    rt.view.borrow_mut().levels.truncate(depth);
}

/// Hides the pooled popup nodes from `depth` outward and forgets what the
/// ones rolling shut showed.
fn hide_pooled_from<M: 'static>(rt: &Runtime<M>, depth: usize) {
    rt.view.borrow_mut().closing.retain(|(at, _)| *at < depth);
    let count = rt.pool.borrow().len();
    for at in depth..count {
        let id = rt.pool.borrow().get(at).copied();
        let unroll = rt.unrolls.borrow().get(at).cloned();
        if let (Some(id), Some(unroll)) = (id, unroll) {
            unroll.hide(&rt.ui, id);
        }
    }
}

/// Closes every popup and clears the bar's open highlight.
pub(super) fn close_all<M: 'static>(rt: &Runtime<M>) {
    close_levels_from(rt, 0);
    clear_bar(rt);
}

/// Hides every popup at once and clears the bar's open highlight.
fn hide_all<M: 'static>(rt: &Runtime<M>) {
    hide_levels_from(rt, 0);
    clear_bar(rt);
}

/// Clears the bar's open highlight.
fn clear_bar<M: 'static>(rt: &Runtime<M>) {
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
        focus_node(rt, id);
        rt.ui.invalidate(id);
    }
}

/// Restores [`Runtime::moving_focus`] when a focus move ends, even by a panic.
struct FocusMove<'a> {
    flag: &'a Cell<bool>,
    previous: bool,
}

impl Drop for FocusMove<'_> {
    fn drop(&mut self) {
        self.flag.set(self.previous);
    }
}

/// Gives the keyboard focus to one of the menu's own nodes.
///
/// The node losing the focus is told `KillFocus`, which [`dismiss`] reads as
/// the user clicking elsewhere; the guard marks this move as the menu's own.
pub(super) fn focus_node<M: 'static>(rt: &Runtime<M>, id: WidgetId) {
    let _move = FocusMove {
        previous: rt.moving_focus.replace(true),
        flag: &rt.moving_focus,
    };
    rt.ui.focus(id);
}

/// Closes the menu because a popup or the bar lost the keyboard focus, unless
/// it went to another of the menu's own nodes (a press on the bar, or on a
/// parent level while a submenu holds it) or the menu moved it itself.
pub(super) fn dismiss<M: 'static>(rt: &Runtime<M>) {
    if rt.moving_focus.get() {
        return;
    }
    let holder = rt.ui.focus_holder();
    let own = |id| rt.bar.get() == Some(id) || rt.pool.borrow().contains(&id);
    if holder.is_some_and(own) {
        return;
    }
    close_all(rt);
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
