#![forbid(unsafe_code)]

//! Mounting a [`Layout`]: placing it on a window or container and keeping it
//! placed.

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use super::Layout;
use crate::app::{LayoutHook, Ui};
use crate::backend::{Event, Result, WidgetId};
use crate::geometry::{Rect, Size};
use crate::layout::{Constraints, Group, Leaf};
use crate::widget::Placeable;

/// Where a mounted layout gets its area from.
#[derive(Clone, Copy)]
pub(super) enum Area {
    /// The window's client area.
    Window,
    /// A container node's own extent (its children use its coordinates).
    Node(WidgetId),
    /// Whatever area the container node that owns it places it in (a scroll
    /// view's content); it never lays itself out.
    Driven(WidgetId),
}

pub(super) struct State<M: 'static> {
    pub(super) ui: Ui<M>,
    pub(super) area: Area,
    pub(super) tree: Group<usize>,
    pub(super) widgets: Vec<Rc<dyn Placeable<M>>>,
    /// The area of the last placement.
    pub(super) placed_in: Cell<Rect>,
    /// Widgets the layout hid because a frame around them is hidden; they
    /// show again with the frame.
    collapsed: RefCell<Vec<bool>>,
    /// Set while placing, so a change the placement causes cannot re-enter it.
    placing: Cell<bool>,
}

impl<M: 'static> State<M> {
    fn area_rect(&self) -> Option<Rect> {
        match self.area {
            Area::Window => Some(self.ui.client_rect()),
            Area::Node(id) => Some(Rect::from_size(self.ui.bounds(id).size())),
            Area::Driven(_) => None,
        }
    }

    pub(super) fn leaf(&self, key: &usize, constraints: Constraints) -> Leaf {
        let widget = &self.widgets[*key];
        let id = widget.id();
        Leaf {
            natural: widget.measure(&self.ui, constraints),
            visible: id.is_none() || self.ui.is_visible(id),
            content: widget.content_insets(&self.ui),
        }
    }

    fn relayout(&self) {
        if let Some(area) = self.area_rect() {
            self.place(area);
        }
    }

    /// Places the tree in `area`.
    fn place(&self, area: Rect) {
        if self.placing.replace(true) {
            return;
        }
        self.placed_in.set(area);
        if !area.is_empty() {
            let dpi = self.ui.dpi();
            let placed = self.tree.compute(area, dpi, &|key, c| self.leaf(key, c));
            let mut moves = Vec::with_capacity(placed.len());
            let mut after = Vec::with_capacity(placed.len());
            let mut shown = vec![false; self.widgets.len()];
            for (key, rect) in placed {
                shown[key] = true;
                let id = self.widgets[key].id();
                if id.is_none() {
                    continue;
                }
                moves.push((id, rect));
                after.push((key, rect));
            }
            self.collapse(&shown);
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

    /// Hides the widgets the tree left out although the app shows them (the
    /// content of a hidden frame), and shows again the ones it placed after
    /// collapsing them.
    fn collapse(&self, shown: &[bool]) {
        let mut collapsed = self.collapsed.borrow_mut();
        for (key, widget) in self.widgets.iter().enumerate() {
            let id = widget.id();
            if id.is_none() {
                continue;
            }
            // Read once: the app's own choice, which the layout never overrides.
            let wanted = self.ui.is_visible(id);
            let hide = !shown[key] && wanted;
            if hide != collapsed[key] {
                collapsed[key] = hide;
                // Show again only what the layout hid and the app still shows.
                if hide || wanted {
                    self.ui.show_node(id, !hide);
                }
            }
        }
    }

    /// The size the tree wants within `constraints`.
    fn measure(&self, constraints: Constraints) -> Size {
        self.tree.measure(constraints, &|key, c| self.leaf(key, c))
    }
}

/// The window's view of a mounted layout.
struct Hook<M: 'static>(Weak<State<M>>);

impl<M: 'static> LayoutHook for Hook<M> {
    fn relayout(&self) {
        if let Some(state) = self.0.upgrade() {
            state.relayout();
        }
    }

    fn report(&self, out: &mut String) {
        if let Some(state) = self.0.upgrade() {
            super::report::write(&state, out);
        }
    }

    fn rects(&self, out: &mut Vec<Rect>) {
        if let Some(state) = self.0.upgrade()
            && matches!(state.area, Area::Window)
        {
            super::report::rects(&state, out);
        }
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
        self.state
            .measure(Constraints::unbounded(self.state.ui.dpi()))
    }

    /// The size the content wants within `constraints`.
    pub(crate) fn measure(&self, constraints: Constraints) -> Size {
        self.state.measure(constraints)
    }

    /// Places the content in `area`, for the container that drives it.
    pub(crate) fn place(&self, area: Rect) {
        self.state.place(area);
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

    /// Creates `layout`'s widgets inside `container`, which places them
    /// itself through [`Mounted::place`] (a scroll view, at its offset).
    pub(crate) fn mount_driven(
        &self,
        container: WidgetId,
        layout: Layout<M>,
    ) -> Result<Mounted<M>> {
        mount(self, Area::Driven(container), layout)
    }
}

fn mount<M: 'static>(ui: &Ui<M>, area: Area, layout: Layout<M>) -> Result<Mounted<M>> {
    let parent = match area {
        Area::Window => ui.clone(),
        Area::Node(id) | Area::Driven(id) => ui.with_parent(id),
    };
    let mut widgets = Vec::new();
    let tree = layout.realize(&parent, &mut widgets)?;
    let count = widgets.len();
    let state = Rc::new(State {
        ui: ui.clone(),
        area,
        tree,
        widgets,
        placed_in: Cell::new(Rect::default()),
        collapsed: RefCell::new(vec![false; count]),
        placing: Cell::new(false),
    });

    let weak: Weak<State<M>> = Rc::downgrade(&state);
    let hook = ui.add_layout_hook(Rc::new(Hook(weak.clone())));
    let listener = match area {
        Area::Window | Area::Driven(_) => None,
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
