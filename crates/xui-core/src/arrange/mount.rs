#![forbid(unsafe_code)]

//! Mounting a [`Layout`]: placing it on a window or container and keeping it
//! placed.

use std::cell::Cell;
use std::rc::{Rc, Weak};

use super::Layout;
use crate::app::Ui;
use crate::backend::{Event, Result, WidgetId};
use crate::geometry::{Rect, Size};
use crate::layout::{Constraints, Group, Leaf};
use crate::widget::Placeable;

/// Where a mounted layout gets its area from.
#[derive(Clone, Copy)]
enum Area {
    /// The window's client area.
    Window,
    /// A container node's own extent (its children use its coordinates).
    Node(WidgetId),
}

struct State<M: 'static> {
    ui: Ui<M>,
    area: Area,
    tree: Group<usize>,
    widgets: Vec<Rc<dyn Placeable<M>>>,
    /// Set while placing, so a change the placement causes cannot re-enter it.
    placing: Cell<bool>,
}

impl<M: 'static> State<M> {
    fn area_rect(&self) -> Rect {
        match self.area {
            Area::Window => self.ui.client_rect(),
            Area::Node(id) => Rect::from_size(self.ui.bounds(id).size()),
        }
    }

    fn leaf(&self, key: &usize, constraints: Constraints) -> Leaf {
        let widget = &self.widgets[*key];
        let id = widget.id();
        Leaf {
            natural: widget.measure(&self.ui, constraints),
            visible: id.is_none() || self.ui.is_visible(id),
            content: widget.content_insets(&self.ui),
        }
    }

    fn relayout(&self) {
        if self.placing.replace(true) {
            return;
        }
        let area = self.area_rect();
        if !area.is_empty() {
            let dpi = self.ui.dpi();
            let placed = self.tree.compute(area, dpi, &|key, c| self.leaf(key, c));
            let mut moves = Vec::with_capacity(placed.len());
            let mut after = Vec::with_capacity(placed.len());
            for (key, rect) in placed {
                let id = self.widgets[key].id();
                if id.is_none() {
                    continue;
                }
                moves.push((id, rect));
                after.push((key, rect));
            }
            // One batch for the whole tree, so a relayout does not flicker.
            self.ui.apply_moves(&moves);
            // Satellite nodes (a list's scrollbar) follow once the primary
            // nodes are placed, so they read the new bounds.
            for (key, rect) in after {
                self.widgets[key].placed(&self.ui, rect);
            }
        }
        self.placing.set(false);
    }
}

/// A layout placed on a window or container. Dropping it destroys the widgets
/// it owns and stops the re-flow, so the app keeps it for as long as the
/// window's content should live (one field replaces a field per widget).
pub struct Mounted<M: 'static> {
    state: Rc<State<M>>,
    hook: usize,
    /// The container listener to remove, for a layout mounted in a node.
    listener: Option<(WidgetId, usize)>,
}

impl<M: 'static> Mounted<M> {
    /// Lays the widgets out again now.
    pub fn relayout(&self) {
        self.state.relayout();
    }

    /// The size the content wants, in device pixels at the window's DPI: use it
    /// to choose an initial window size. A `fill` entry adds no natural extent.
    pub fn preferred_size(&self) -> Size {
        let dpi = self.state.ui.dpi();
        self.state
            .tree
            .preferred_size(dpi, &|key, c| self.state.leaf(key, c))
    }
}

impl<M: 'static> Drop for Mounted<M> {
    fn drop(&mut self) {
        self.state.ui.remove_layout_hook(self.hook);
        if let Some((id, token)) = self.listener {
            self.state.ui.remove_events(id, token);
        }
    }
}

impl<M: 'static> Ui<M> {
    /// Creates `layout`'s widgets as the window's content and keeps them
    /// placed in its client area as it resizes, changes DPI or shows and hides
    /// widgets, for as long as the window lives. Fails with the first widget
    /// constructor error.
    pub fn root(&self, layout: Layout<M>) -> Result<()> {
        let mounted = self.mount(layout)?;
        self.retain(Box::new(mounted));
        Ok(())
    }

    /// Like [`Ui::root`], but returns the [`Mounted`] layout instead of
    /// keeping it: dropping it destroys the widgets, for content that is
    /// replaced while the window lives.
    pub fn mount(&self, layout: Layout<M>) -> Result<Mounted<M>> {
        mount(self, Area::Window, layout)
    }

    /// Creates `layout`'s widgets inside the container node `container` (a
    /// `Panel`, say) and keeps them placed in it, re-flowing when the
    /// container is resized.
    pub fn mount_in(&self, container: WidgetId, layout: Layout<M>) -> Result<Mounted<M>> {
        mount(self, Area::Node(container), layout)
    }
}

fn mount<M: 'static>(ui: &Ui<M>, area: Area, layout: Layout<M>) -> Result<Mounted<M>> {
    let parent = match area {
        Area::Window => ui.clone(),
        Area::Node(id) => ui.with_parent(id),
    };
    let mut widgets = Vec::new();
    let tree = layout.realize(&parent, &mut widgets)?;
    let state = Rc::new(State {
        ui: ui.clone(),
        area,
        tree,
        widgets,
        placing: Cell::new(false),
    });

    let weak: Weak<State<M>> = Rc::downgrade(&state);
    let hook = ui.add_layout_hook({
        let weak = weak.clone();
        move || {
            if let Some(state) = weak.upgrade() {
                state.relayout();
            }
        }
    });
    let listener = match area {
        Area::Window => None,
        Area::Node(id) => {
            let token = ui.add_events(id, move |event| {
                if let (Event::Resize { .. }, Some(state)) = (event, weak.upgrade()) {
                    state.relayout();
                }
                None
            });
            Some((id, token))
        }
    };

    state.relayout();
    Ok(Mounted {
        state,
        hook,
        listener,
    })
}
