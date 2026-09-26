#![forbid(unsafe_code)]

//! [`Menu`]: portable menu bars and popup/context menus.
//!
//! A menu is a tree of entries with opaque [`MenuId`]s. A bar horizontalizes
//! its top-level entries; opening one drops a vertical popup below it, and a
//! submenu opens a further popup to its side. A context menu is a popup with no
//! bar, shown at a client point with [`Menu::show_context`].
//!
//! Commands raise the app's `Msg` through closures given at construction:
//! [`Menu::on_select`] for plain items and [`Menu::on_toggle`] for check and
//! radio items, which also carry their new checked state. Entries travel by
//! mouse or keyboard — arrows, Enter, Escape, and a letter underlines and
//! activates a mnemonic.
//!
//! The popups are pooled, one node per nesting level, and shown/hidden through
//! the backend the way [`ComboBox`](super::ComboBox) shows its list.

mod events;
mod layout;
mod model;
mod open;
mod paint;

#[cfg(test)]
mod tests;

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use super::control::Control;
use crate::app::Ui;
use crate::backend::{NodeKind, NodeSpec, Result, WidgetId};
use crate::geometry::{Point, Rect};
use crate::property::{Properties, Property, Value};

use model::Node;

/// An opaque handle to one [`Menu`] entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MenuId(usize);

impl MenuId {
    /// Wraps `value`. Distinct entries must get distinct ids.
    pub const fn new(value: usize) -> MenuId {
        MenuId(value)
    }
}

/// The sentinel id of a [`separator`](MenuScope::separator).
const SEPARATOR: MenuId = MenuId(usize::MAX);

/// Maps a chosen command to an optional app message.
type SelectMapper<M> = RefCell<Option<Box<dyn Fn(MenuId) -> Option<M>>>>;
/// Maps a check/radio toggle to an optional app message.
type ToggleMapper<M> = RefCell<Option<Box<dyn Fn(MenuId, bool) -> Option<M>>>>;

/// The entries of one menu while it is being built.
///
/// Handed to [`Menu::build`] and to a [`submenu`](MenuScope::submenu)'s
/// closure; every call appends one entry.
pub struct MenuScope<'a> {
    entries: &'a mut Vec<Node>,
}

impl MenuScope<'_> {
    /// Appends a command that raises its id when chosen.
    pub fn item(&mut self, id: MenuId, text: &str) -> &mut Self {
        self.entries.push(Node::command(id, text));
        self
    }

    /// Appends a divider between groups of entries.
    pub fn separator(&mut self) -> &mut Self {
        self.entries.push(Node::separator());
        self
    }

    /// Appends a checkable command.
    pub fn check(&mut self, id: MenuId, text: &str, checked: bool) -> &mut Self {
        self.entries.push(Node::check(id, text, checked));
        self
    }

    /// Appends a radio command; picking one clears its radio siblings.
    pub fn radio(&mut self, id: MenuId, text: &str, checked: bool) -> &mut Self {
        self.entries.push(Node::radio(id, text, checked));
        self
    }

    /// Appends an entry that opens `build`'s entries as a submenu.
    pub fn submenu(
        &mut self,
        id: MenuId,
        text: &str,
        build: impl FnOnce(&mut MenuScope<'_>),
    ) -> &mut Self {
        let mut node = Node::submenu(id, text);
        build(&mut MenuScope {
            entries: &mut node.children,
        });
        self.entries.push(node);
        self
    }
}

/// The interactive state a popup painter reads.
///
/// It holds no backend handle, so a painter can share it without keeping the
/// window alive.
#[derive(Default)]
struct View {
    /// The bar title the pointer or keyboard highlights, if any.
    pub(super) bar_hover: Option<usize>,
    /// The bar title whose menu is open, if any.
    pub(super) bar_open: Option<usize>,
    /// One entry per open popup, outermost first.
    pub(super) levels: Vec<Level>,
}

/// The path and highlight of one open popup.
struct Level {
    /// The index path from the root to the displayed entry list.
    pub(super) path: Vec<usize>,
    /// The highlighted row, if any.
    pub(super) hover: Option<usize>,
}

/// One open popup: its pooled node and where it sits in client coordinates.
struct Open {
    /// The popup's node.
    pub(super) id: WidgetId,
    /// The popup's client rectangle.
    pub(super) bounds: Rect,
}

/// The state the menu's painters and event mappers share.
struct Runtime<M: 'static> {
    /// The window handle.
    pub(super) ui: Ui<M>,
    /// The entry tree.
    pub(super) model: Rc<RefCell<Vec<Node>>>,
    /// The open/hover state painters read.
    pub(super) view: Rc<RefCell<View>>,
    /// The open popups, parallel to `view.levels`.
    pub(super) open: RefCell<Vec<Open>>,
    /// The pooled popup nodes, indexed by nesting depth.
    pub(super) pool: RefCell<Vec<WidgetId>>,
    /// Maps a chosen command to an app message.
    pub(super) on_select: SelectMapper<M>,
    /// Maps a check/radio toggle to an app message.
    pub(super) on_toggle: ToggleMapper<M>,
    /// The bar's node, when this menu has one.
    pub(super) bar: Cell<Option<WidgetId>>,
}

impl<M: 'static> Runtime<M> {
    fn new(ui: &Ui<M>) -> Rc<Runtime<M>> {
        Rc::new(Runtime {
            ui: ui.clone(),
            model: Rc::new(RefCell::new(Vec::new())),
            view: Rc::new(RefCell::new(View::default())),
            open: RefCell::new(Vec::new()),
            pool: RefCell::new(Vec::new()),
            on_select: RefCell::new(None),
            on_toggle: RefCell::new(None),
            bar: Cell::new(None),
        })
    }
}

/// A portable menu: a bar of top-level entries, or a standalone popup.
pub struct Menu<M: 'static> {
    rt: Rc<Runtime<M>>,
    bar: Option<Control<M>>,
    popups: Vec<Control<M>>,
}

impl<M: 'static> Menu<M> {
    /// Creates a menu bar of `bounds`; fill it with [`Menu::build`].
    pub fn bar(ui: &Ui<M>, bounds: Rect) -> Result<Menu<M>> {
        let control = Control::new(ui, &NodeSpec::new(NodeKind::Toolbar, bounds).tab_stop())?;
        let rt = Runtime::new(ui);
        rt.bar.set(Some(control.id()));
        {
            let model = Rc::clone(&rt.model);
            let view = Rc::clone(&rt.view);
            let theme = ui.theme_handle();
            let selected = control.selected_handle();
            control.set_painter(Rc::new(move |canvas| {
                paint::bar(
                    &model.borrow(),
                    &view.borrow(),
                    canvas,
                    theme.get(),
                    selected.get(),
                );
            }));
        }
        {
            let rt = Rc::clone(&rt);
            control.on_events(move |event| events::bar(&rt, event));
        }
        Ok(Menu {
            rt,
            bar: Some(control),
            popups: Vec::new(),
        })
    }

    /// Creates a context menu with no bar; fill it with [`Menu::build`] and
    /// show it with [`Menu::show_context`].
    pub fn context(ui: &Ui<M>) -> Menu<M> {
        Menu {
            rt: Runtime::new(ui),
            bar: None,
            popups: Vec::new(),
        }
    }

    /// Fills the menu's entries and pools the popups the tree needs.
    pub fn build(self, fill: impl FnOnce(&mut MenuScope<'_>)) -> Menu<M> {
        {
            let mut entries = self.rt.model.borrow_mut();
            fill(&mut MenuScope {
                entries: &mut entries,
            });
        }
        let depth = {
            let model = self.rt.model.borrow();
            if self.bar.is_some() {
                layout::bar_depth(&model)
            } else {
                layout::level_count(&model)
            }
        };
        let mut popups = Vec::with_capacity(depth);
        for level in 0..depth {
            let Ok(control) = Control::new(
                &self.rt.ui,
                &NodeSpec::new(NodeKind::Custom, Rect::default()),
            ) else {
                break;
            };
            self.rt.pool.borrow_mut().push(control.id());
            {
                let model = Rc::clone(&self.rt.model);
                let view = Rc::clone(&self.rt.view);
                let theme = self.rt.ui.theme_handle();
                control.set_painter(Rc::new(move |canvas| {
                    paint::popup(&model.borrow(), &view.borrow(), level, canvas, theme.get());
                }));
            }
            {
                let rt = Rc::clone(&self.rt);
                control.on_events(move |event| events::popup(&rt, level, event));
            }
            self.rt.ui.set_visible(control.id(), false);
            popups.push(control);
        }
        if let Some(bar) = self.rt.bar.get() {
            self.rt.ui.invalidate(bar);
        }
        Menu {
            rt: self.rt,
            bar: self.bar,
            popups,
        }
    }

    /// Maps a chosen plain command to the app's message.
    pub fn on_select(self, mapper: impl Fn(MenuId) -> Option<M> + 'static) -> Menu<M> {
        *self.rt.on_select.borrow_mut() = Some(Box::new(mapper));
        self
    }

    /// Maps a check/radio toggle to the app's message, carrying the new state.
    pub fn on_toggle(self, mapper: impl Fn(MenuId, bool) -> Option<M> + 'static) -> Menu<M> {
        *self.rt.on_toggle.borrow_mut() = Some(Box::new(mapper));
        self
    }

    /// The bar's node identity, if this menu has a bar.
    pub fn id(&self) -> Option<WidgetId> {
        self.bar.as_ref().map(Control::id)
    }

    /// The pooled node that shows the popup at nesting `depth`, if the tree is
    /// deep enough to need one. Depth `0` is the bar menu or the context popup.
    pub fn popup_id(&self, depth: usize) -> Option<WidgetId> {
        self.popups.get(depth).map(Control::id)
    }

    /// Whether any popup is open.
    pub fn is_open(&self) -> bool {
        !self.rt.open.borrow().is_empty()
    }

    /// Closes every popup.
    pub fn close(&self) {
        open::close_all(&self.rt);
    }

    /// Shows the root entries as a context popup at the client point `(x, y)`.
    pub fn show_context(&self, x: i32, y: i32) {
        open::show_context(&self.rt, Point::new(x, y));
    }

    /// Enables or disables entry `id`. A disabled entry is dimmed and ignores
    /// input.
    pub fn set_enabled(&self, id: MenuId, enabled: bool) {
        if model::set_enabled(&mut self.rt.model.borrow_mut(), id, enabled) {
            open::redraw(&self.rt);
        }
    }

    /// Whether entry `id` is enabled.
    pub fn is_enabled(&self, id: MenuId) -> bool {
        model::find(&self.rt.model.borrow(), id).is_some_and(|node| node.enabled)
    }

    /// Sets a check/radio entry's state without raising an event.
    pub fn set_checked(&self, id: MenuId, checked: bool) {
        if model::set_checked(&mut self.rt.model.borrow_mut(), id, checked) {
            open::redraw(&self.rt);
        }
    }

    /// Whether a check/radio entry is checked.
    pub fn is_checked(&self, id: MenuId) -> bool {
        model::find(&self.rt.model.borrow(), id).is_some_and(|node| node.checked)
    }

    /// Marks the bar selected, so its painter draws an outline (a form
    /// editor's selection).
    pub fn set_selected(&self, selected: bool) {
        if let Some(bar) = &self.bar {
            bar.set_selected(selected);
        }
    }
}

impl<M: 'static> Properties for Menu<M> {
    fn properties(&self) -> Vec<Property> {
        vec![Property {
            name: "open",
            value: Value::Bool(self.is_open()),
        }]
    }

    fn set_property(&self, name: &str, value: Value) -> bool {
        match (name, value) {
            ("open", Value::Bool(open)) => {
                if open {
                    open::open_bar(&self.rt, 0);
                } else {
                    self.close();
                }
                true
            }
            _ => false,
        }
    }
}
