#![forbid(unsafe_code)]

//! [`Tabs`]: a paged container with a tab strip.
//!
//! Each page is a list of child widgets, created through [`Tabs::ui`] and
//! registered with [`Tabs::page`]. The selected page's children fill the page
//! area below the strip; every other page's children are hidden, so they take
//! no space and cannot receive focus. [`Tabs::on_change`] maps a selection to
//! the app's message.

mod pages;
mod strip;
#[cfg(test)]
mod tests;

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use self::strip::{Underline, paint_strip, select_tab, strip_event};
use super::Panel;
use super::control::Control;
use crate::app::Ui;
use crate::arrange::Mounted;
use crate::backend::{NodeKind, NodeSpec, Result, TextStyle, WidgetId};
use crate::geometry::Rect;
use crate::layout::Dock;
use crate::property::{Properties, Property, Value, node};
use crate::theme::look::backdrop;
use crate::units::Dip;

/// The strip's height.
const STRIP: Dip = Dip(32.0);
/// Horizontal padding around a tab title.
const PADDING: Dip = Dip(12.0);
/// The narrowest a tab may be.
const MIN_TAB: Dip = Dip(48.0);
/// The design size of a tab title.
const TEXT: Dip = Dip(12.0);
/// The height of the selected tab's accent underline.
const ACCENT: i32 = 2;

type ChangeMapper<M> = RefCell<Option<Box<dyn Fn(usize) -> Option<M>>>>;

/// State the container, its strip and their painters and mappers share.
struct Shared<M: 'static> {
    id: WidgetId,
    strip_id: WidgetId,
    titles: RefCell<Vec<String>>,
    pages: RefCell<Vec<Vec<WidgetId>>>,
    /// One rectangle per tab, in the strip's own coordinates.
    tabs: RefCell<Vec<Rect>>,
    selected: Cell<usize>,
    hover: Cell<Option<usize>>,
    /// Where the selected tab's accent underline is, travelling between tabs.
    underline: Underline,
    on_change: ChangeMapper<M>,
}

/// A paged container with a tab strip.
pub struct Tabs<M: 'static> {
    control: Control<M>,
    _strip: Control<M>,
    scoped: Ui<M>,
    shared: Rc<Shared<M>>,
    /// The panel and mounted layout behind each page added with
    /// [`Tabs::add_layout_page`].
    layouts: RefCell<Vec<(Panel<M>, Mounted<M>)>>,
}

impl<M: 'static> Tabs<M> {
    /// Creates an empty tabs container at `bounds`.
    pub(crate) fn new(ui: &Ui<M>, bounds: Rect) -> Result<Tabs<M>> {
        let control = Control::new(ui, &NodeSpec::new(NodeKind::Tabs, bounds))?;
        let scoped = ui.with_parent(control.id());
        let strip = Control::new(
            &scoped,
            &NodeSpec::new(NodeKind::Container, Rect::default()),
        )?;
        let shared = Rc::new(Shared {
            id: control.id(),
            strip_id: strip.id(),
            titles: RefCell::new(Vec::new()),
            pages: RefCell::new(Vec::new()),
            tabs: RefCell::new(Vec::new()),
            selected: Cell::new(0),
            hover: Cell::new(None),
            underline: Underline::new(),
            on_change: RefCell::new(None),
        });
        {
            let theme = ui.theme_handle();
            control.set_painter(Rc::new(move |canvas| {
                backdrop(canvas, theme.get().background)
            }));
        }
        {
            let shared = Rc::clone(&shared);
            let theme = ui.theme_handle();
            let time = ui.frame_time_handle();
            strip.set_painter(Rc::new(move |canvas| {
                paint_strip(&shared, canvas, theme.get(), time.get())
            }));
        }
        {
            let shared = Rc::clone(&shared);
            let ui = scoped.clone();
            strip.on_events(move |event| strip_event(&shared, &ui, event));
        }
        Ok(Tabs {
            control,
            _strip: strip,
            scoped,
            shared,
            layouts: RefCell::new(Vec::new()),
        })
    }

    /// The handle widgets built inside this container parent to.
    pub fn ui(&self) -> &Ui<M> {
        &self.scoped
    }

    /// Puts the widgets inside this tab control in or out of design mode: they
    /// ignore their own input while everything outside stays live. Nested
    /// containers inherit it.
    pub fn set_design_mode(&self, on: bool) {
        self.ui().set_design_mode(on);
    }

    /// The container's node identity.
    pub fn id(&self) -> WidgetId {
        self.control.id()
    }

    /// Appends `title`'s page, showing `children` when it is selected.
    pub fn page(self, title: &str, children: &[WidgetId]) -> Tabs<M> {
        self.add_page(title, children);
        self
    }

    /// The number of pages.
    pub fn page_count(&self) -> usize {
        self.shared.pages.borrow().len()
    }

    /// Appends a page at runtime, like [`Tabs::page`] on a handle already in
    /// use. Selecting it is left to [`Tabs::select`].
    pub fn add_page(&self, title: &str, children: &[WidgetId]) {
        let s = &self.shared;
        s.titles.borrow_mut().push(title.to_string());
        s.pages.borrow_mut().push(children.to_vec());
        relayout(&self.scoped, s);
    }

    /// Removes the page at `index` and returns its children (now hidden), so
    /// the caller can destroy them; `None` when `index` is out of range.
    ///
    /// The selection follows the page it was on: removing an earlier page
    /// shifts it down, and removing the selected page selects the page that
    /// took its place (the last one if it was last). No change event is
    /// raised, as with [`Tabs::select`].
    pub fn remove_page(&self, index: usize) -> Option<Vec<WidgetId>> {
        let s = &self.shared;
        if index >= s.pages.borrow().len() {
            return None;
        }
        s.titles.borrow_mut().remove(index);
        let children = s.pages.borrow_mut().remove(index);
        for &child in &children {
            self.scoped.set_visible(child, false);
        }
        let remaining = s.pages.borrow().len();
        let selected = s.selected.get();
        if index < selected {
            s.selected.set(selected - 1);
        } else if selected >= remaining {
            s.selected.set(remaining.saturating_sub(1));
        }
        s.hover.set(None);
        relayout(&self.scoped, s);
        Some(children)
    }

    /// Renames the page at `index`; `false` when `index` is out of range.
    pub fn rename_page(&self, index: usize, title: &str) -> bool {
        match self.shared.titles.borrow_mut().get_mut(index) {
            Some(slot) => *slot = title.to_string(),
            None => return false,
        }
        relayout(&self.scoped, &self.shared);
        true
    }

    /// Maps a selection to the app's message: the closure receives the index
    /// and returns `Some(msg)` to raise it, or `None` to ignore it.
    pub fn on_change(self, f: impl Fn(usize) -> Option<M> + 'static) -> Tabs<M> {
        *self.shared.on_change.borrow_mut() = Some(Box::new(f));
        self
    }

    /// The selected page's index.
    pub fn selected(&self) -> usize {
        self.shared.selected.get()
    }

    /// Selects `index` programmatically without raising the change event.
    pub fn select(&self, index: usize) {
        select_tab(&self.scoped, &self.shared, index);
    }

    /// Moves/resizes the container and re-lays the pages out.
    pub(crate) fn set_bounds(&self, bounds: Rect) {
        self.control.set_bounds(bounds);
        relayout(&self.scoped, &self.shared);
    }

    /// Re-lays the pages out from the container's current bounds.
    pub fn relayout(&self) {
        relayout(&self.scoped, &self.shared);
    }

    /// Shows or hides the container.
    pub fn set_visible(&self, visible: bool) {
        self.control.set_visible(visible);
    }

    /// Enables or disables the container.
    pub fn set_enabled(&self, enabled: bool) {
        self.control.set_enabled(enabled);
    }
}

impl<M: 'static> Properties for Tabs<M> {
    fn properties(&self) -> Vec<Property> {
        vec![
            Property {
                name: "selected",
                value: Value::Integer(self.selected() as i64),
            },
            node::enabled(self.control.ui(), self.id()),
            node::visible(self.control.ui(), self.id()),
        ]
    }

    fn set_property(&self, name: &str, value: Value) -> bool {
        if node::set_visible(self.control.ui(), self.id(), name, &value) {
            return true;
        }
        match (name, value) {
            ("selected", Value::Integer(index)) if index >= 0 => {
                self.select(index as usize);
                true
            }
            ("enabled", Value::Bool(enabled)) => {
                self.set_enabled(enabled);
                true
            }
            _ => false,
        }
    }
}

/// [`lay_out`], then puts the underline under the selected tab at once.
fn relayout<M>(ui: &Ui<M>, s: &Shared<M>) {
    if let Some(tab) = lay_out(ui, s) {
        s.underline.settle(tab);
    }
}

/// Carves the strip off the top, sizes the tabs and places the selected page.
/// Returns the selected tab's rectangle, or `None` when the container has no
/// bounds yet or there is no such tab.
fn lay_out<M>(ui: &Ui<M>, s: &Shared<M>) -> Option<Rect> {
    // Visibility does not depend on bounds, so apply it even while the
    // container has none.
    let selected = s.selected.get();
    for (index, children) in s.pages.borrow().iter().enumerate() {
        for &child in children {
            ui.set_visible(child, index == selected);
        }
    }
    let node = ui.bounds(s.id);
    if node.is_empty() {
        return None;
    }
    // Children are parented to the container, so they are placed in its own
    // coordinates: start the strip and page at the origin.
    let bounds = Rect::from_size(node.size());
    let dpi = ui.dpi();
    let areas = Dock::new().top(STRIP).split(bounds, dpi);
    let strip = areas.top.unwrap_or(bounds);
    let page = areas.fill;

    let style = TextStyle::new(ui.theme().text, TEXT);
    let padding = PADDING.to_px(dpi).value().max(0);
    let minimum = MIN_TAB.to_px(dpi).value().max(0);
    let titles = s.titles.borrow();
    let mut x = 0;
    let mut tabs = Vec::with_capacity(titles.len());
    for title in titles.iter() {
        let width = (ui.measure_text(title, &style, dpi).width + padding * 2).max(minimum);
        tabs.push(Rect::new(x, 0, x + width, strip.height()));
        x += width;
    }
    drop(titles);
    let selected_tab = tabs.get(selected).copied();
    *s.tabs.borrow_mut() = tabs;

    let mut moves: Vec<(WidgetId, Rect)> = vec![(s.strip_id, strip)];
    if let Some(children) = s.pages.borrow().get(selected) {
        moves.extend(children.iter().map(|&child| (child, page)));
    }
    ui.apply_moves(&moves);
    ui.invalidate(s.strip_id);
    selected_tab
}
