#![forbid(unsafe_code)]

//! [`Tabs`]: a paged container with a tab strip.
//!
//! Each page is a list of child widgets, created through [`Tabs::ui`] and
//! registered with [`Tabs::page`]. The selected page's children fill the page
//! area below the strip; every other page's children are hidden, so they take
//! no space and cannot receive focus. [`Tabs::on_change`] maps a selection to
//! the app's message.

#[cfg(test)]
mod tests;

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use super::control::Control;
use crate::app::Ui;
use crate::backend::{Canvas, Event, NodeKind, NodeSpec, Result, TextStyle, WidgetId};
use crate::geometry::{Point, Rect};
use crate::layout::Dock;
use crate::message::MouseButton;
use crate::property::{Properties, Property, Value};
use crate::theme::Theme;
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
    on_change: ChangeMapper<M>,
}

/// A paged container with a tab strip.
pub struct Tabs<M: 'static> {
    control: Control<M>,
    _strip: Control<M>,
    scoped: Ui<M>,
    shared: Rc<Shared<M>>,
}

impl<M: 'static> Tabs<M> {
    /// Creates an empty tabs container at `bounds`.
    pub fn new(ui: &Ui<M>, bounds: Rect) -> Result<Tabs<M>> {
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
            on_change: RefCell::new(None),
        });
        {
            let theme = ui.theme_handle();
            control.set_painter(Rc::new(move |canvas| canvas.clear(theme.get().background)));
        }
        {
            let shared = Rc::clone(&shared);
            let theme = ui.theme_handle();
            strip.set_painter(Rc::new(move |canvas| {
                paint_strip(&shared, canvas, theme.get())
            }));
        }
        {
            let shared = Rc::clone(&shared);
            let ui = ui.clone();
            strip.on_events(move |event| strip_event(&shared, &ui, event));
        }
        Ok(Tabs {
            control,
            _strip: strip,
            scoped,
            shared,
        })
    }

    /// The handle widgets built inside this container parent to.
    pub fn ui(&self) -> &Ui<M> {
        &self.scoped
    }

    /// The container's node identity.
    pub fn id(&self) -> WidgetId {
        self.control.id()
    }

    /// Appends `title`'s page, showing `children` when it is selected.
    pub fn page(self, title: &str, children: &[WidgetId]) -> Tabs<M> {
        self.shared.titles.borrow_mut().push(title.to_string());
        self.shared.pages.borrow_mut().push(children.to_vec());
        relayout(&self.scoped, &self.shared);
        self
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
        if index >= self.shared.pages.borrow().len() || self.shared.selected.get() == index {
            return;
        }
        self.shared.selected.set(index);
        relayout(&self.scoped, &self.shared);
    }

    /// Moves/resizes the container and re-lays the pages out.
    pub fn set_bounds(&self, bounds: Rect) {
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
        vec![Property {
            name: "selected",
            value: Value::Integer(self.selected() as i64),
        }]
    }

    fn set_property(&self, name: &str, value: Value) -> bool {
        match (name, value) {
            ("selected", Value::Integer(index)) if index >= 0 => {
                self.select(index as usize);
                true
            }
            _ => false,
        }
    }
}

/// Carves the strip off the top, sizes the tabs and places the selected page.
fn relayout<M>(ui: &Ui<M>, s: &Shared<M>) {
    let node = ui.bounds(s.id);
    if node.is_empty() {
        return;
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
    *s.tabs.borrow_mut() = tabs;

    let mut moves: Vec<(WidgetId, Rect)> = vec![(s.strip_id, strip)];
    let selected = s.selected.get();
    let pages = s.pages.borrow();
    for (index, children) in pages.iter().enumerate() {
        let visible = index == selected;
        for &child in children {
            ui.set_visible(child, visible);
            if visible {
                moves.push((child, page));
            }
        }
    }
    drop(pages);
    ui.apply_moves(&moves);
    ui.invalidate(s.strip_id);
}

/// Selects `index`, re-lays out and raises the change event.
fn choose<M>(ui: &Ui<M>, s: &Shared<M>, index: usize) {
    if index >= s.pages.borrow().len() || s.selected.get() == index {
        return;
    }
    s.selected.set(index);
    s.hover.set(None);
    relayout(ui, s);
    let mapped = s.on_change.borrow().as_ref().and_then(|f| f(index));
    if let Some(msg) = mapped {
        ui.emit(msg);
    }
}

/// The tab under `x` (the strip's own coordinate).
fn tab_at<M>(s: &Shared<M>, x: i32) -> Option<usize> {
    s.tabs
        .borrow()
        .iter()
        .position(|rect| x >= rect.left && x < rect.right)
}

/// Paints the strip: each tab's background, title and the selected underline.
fn paint_strip<M>(s: &Shared<M>, canvas: &mut dyn Canvas, theme: Theme) {
    let bounds = canvas.bounds();
    canvas.clear(theme.surface);
    // Tab rectangles are in the strip's own coordinates; a painter draws in the
    // canvas's surface coordinates, whose origin is the strip's top-left.
    let (ox, oy) = (bounds.left, bounds.top);
    let titles = s.titles.borrow();
    let tabs = s.tabs.borrow();
    for (index, local) in tabs.iter().enumerate() {
        let rect = local.offset(ox, oy);
        let selected = index == s.selected.get();
        if selected {
            canvas.fill_rect(rect, theme.raised);
        } else if s.hover.get() == Some(index) {
            canvas.fill_rect(rect, theme.hover);
        }
        let color = if selected {
            theme.text
        } else {
            theme.text_secondary
        };
        let style = TextStyle::new(color, TEXT).middle();
        let text = Rect::new(
            rect.left,
            rect.top,
            rect.right,
            (rect.bottom - ACCENT).max(rect.top),
        );
        canvas.draw_text(&titles[index], text, &style);
        if selected {
            canvas.fill_rect(
                Rect::new(rect.left, rect.bottom - ACCENT, rect.right, rect.bottom),
                theme.accent,
            );
        }
    }
    drop(tabs);
    drop(titles);
    canvas.draw_line(
        Point::new(bounds.left, bounds.bottom - 1),
        Point::new(bounds.right, bounds.bottom - 1),
        theme.border,
        1.0,
    );
}

/// Handles strip input: hover highlights, a click selects.
fn strip_event<M>(s: &Shared<M>, ui: &Ui<M>, event: &Event) -> Option<M> {
    if ui.is_design_mode() && event.is_input() {
        return None;
    }
    match event {
        Event::MouseMove { x, .. } => {
            let hover = tab_at(s, *x);
            if s.hover.get() != hover {
                s.hover.set(hover);
                ui.invalidate(s.strip_id);
            }
        }
        Event::MouseLeave => {
            if s.hover.replace(None).is_some() {
                ui.invalidate(s.strip_id);
            }
        }
        Event::MouseUp {
            x,
            button: MouseButton::Left,
            ..
        } => {
            if let Some(index) = tab_at(s, *x) {
                choose(ui, s, index);
            }
        }
        _ => {}
    }
    None
}
