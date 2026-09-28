#![forbid(unsafe_code)]

//! [`Toolbar`]: a horizontal strip of clickable items, each an icon, a text
//! label, or both.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use super::control::Control;
use super::tooltip::Tooltip;
use crate::app::Ui;
use crate::backend::{Event, NodeKind, NodeSpec, Result, WidgetId};
use crate::geometry::Rect;
use crate::icon::IconRef;
use crate::message::{Key, MouseButton};
use crate::property::{Properties, Property, Value};

mod paint;

/// Maps the index of a clicked item to an optional app message.
type ClickMapper<M> = Rc<RefCell<Option<Box<dyn Fn(usize) -> Option<M>>>>>;

/// One toolbar entry: an optional icon, an optional label and an optional
/// tooltip. At least one of the icon and the label is shown; a text-only item
/// has no icon.
pub(crate) struct Item {
    /// The item's leading icon.
    pub(crate) icon: Option<IconRef>,
    /// The item's text label, or `None` for an icon-only item.
    pub(crate) label: Option<String>,
    /// The item's hover tooltip, if any.
    pub(crate) tooltip: Option<String>,
}

impl Item {
    /// A text-only item.
    fn text(label: &str) -> Item {
        Item {
            icon: None,
            label: Some(label.to_string()),
            tooltip: None,
        }
    }
}

/// The node-local `[start, end)` span of item `index` in a strip `width`
/// pixels wide holding `count` items.
///
/// Edges are spread proportionally, so every cell stays inside the strip even
/// when it is narrower than the item count (some cells are then empty) and the
/// division remainder is shared out rather than piled onto one cell. Painting
/// and hit-testing both use it, so what is drawn is what is hit.
fn cell_span(width: i32, count: usize, index: usize) -> (i32, i32) {
    if count == 0 || width <= 0 {
        return (0, 0);
    }
    let edge = |i: usize| (i64::from(width) * i as i64 / count as i64) as i32;
    (edge(index), edge(index + 1))
}

/// The item an event `x` (node-local) falls on; `None` outside the strip.
/// `bounds` supplies the strip's total width.
fn item_at(bounds: Rect, x: i32, count: usize) -> Option<usize> {
    let width = bounds.width();
    if count == 0 || width <= 0 || x < 0 || x >= width {
        return None;
    }
    (0..count).find(|&index| {
        let (start, end) = cell_span(width, count, index);
        (start..end).contains(&x)
    })
}

/// The hover tooltip text of item `index`, if it names one.
fn tooltip_at(items: &Rc<RefCell<Vec<Item>>>, index: Option<usize>) -> Option<String> {
    let items = items.borrow();
    index
        .and_then(|index| items.get(index))
        .and_then(|item| item.tooltip.clone())
}

/// A horizontal strip of equally-spaced clickable items.
///
/// An item is a text label, an icon, or an icon with a label, and may name a
/// tooltip. Clicking an item maps its index to the app's `Msg` through
/// [`Toolbar::on_click`]. The left and right arrow keys move a focused item
/// (shown with the hover highlight) and Return activates it; a disabled
/// toolbar ignores input and dims its items.
pub struct Toolbar<M: 'static> {
    control: Control<M>,
    items: Rc<RefCell<Vec<Item>>>,
    hover: Rc<Cell<Option<usize>>>,
    enabled: Rc<Cell<bool>>,
    activated: Rc<Cell<i64>>,
    _tooltip: Option<Rc<Tooltip<M>>>,
    on_click: ClickMapper<M>,
}

impl<M: 'static> Toolbar<M> {
    /// Creates a text-only toolbar of `items` along `bounds`.
    ///
    /// Add icon items to an empty toolbar with [`Toolbar::empty`] and
    /// [`Toolbar::item`].
    pub fn new(ui: &Ui<M>, bounds: Rect, items: &[&str]) -> Result<Toolbar<M>> {
        let toolbar = Toolbar::empty(ui, bounds)?;
        {
            let mut slot = toolbar.items.borrow_mut();
            slot.extend(items.iter().map(|item| Item::text(item)));
        }
        toolbar.control.invalidate();
        Ok(toolbar)
    }

    /// Creates an empty toolbar along `bounds`; add items with [`Toolbar::item`]
    /// and [`Toolbar::item_with_text`].
    pub fn empty(ui: &Ui<M>, bounds: Rect) -> Result<Toolbar<M>> {
        let control = Control::new(ui, &NodeSpec::new(NodeKind::Toolbar, bounds).tab_stop())?;
        let items: Rc<RefCell<Vec<Item>>> = Rc::new(RefCell::new(Vec::new()));
        let hover = Rc::new(Cell::new(None));
        let pressed = Rc::new(Cell::new(None));
        let enabled = Rc::new(Cell::new(true));
        let activated = Rc::new(Cell::new(-1i64));
        let on_click: ClickMapper<M> = Rc::new(RefCell::new(None));

        // One hidden tip for the whole strip; its text follows the hovered item.
        // A toolbar without tooltips simply never shows it.
        let tooltip = Tooltip::attach(ui, control.id(), "").ok().map(Rc::new);

        {
            let items = Rc::clone(&items);
            let hover = Rc::clone(&hover);
            let pressed = Rc::clone(&pressed);
            let enabled = Rc::clone(&enabled);
            let theme = ui.theme_handle();
            let selected = control.selected_handle();
            control.set_painter(Rc::new(move |canvas| {
                paint::paint(
                    canvas,
                    &items,
                    &hover,
                    &pressed,
                    &enabled,
                    &selected,
                    theme.get(),
                );
            }));
        }

        {
            let items = Rc::clone(&items);
            let hover = Rc::clone(&hover);
            let pressed = Rc::clone(&pressed);
            let enabled = Rc::clone(&enabled);
            let activated = Rc::clone(&activated);
            let tooltip = tooltip.clone();
            let on_click = Rc::clone(&on_click);
            let ui = ui.clone();
            let id = control.id();
            control.on_events(move |event| {
                // In design mode the editor handles input, not the widget.
                if ui.is_design_mode() && event.is_input() {
                    return None;
                }
                if !enabled.get() {
                    return None;
                }
                let bounds = ui.bounds(id);
                match event {
                    Event::MouseMove { x, .. } => {
                        let index = item_at(bounds, *x, items.borrow().len());
                        if hover.get() != index {
                            hover.set(index);
                            if let Some(tip) = &tooltip {
                                match tooltip_at(&items, index) {
                                    Some(text) => tip.set_text(&text),
                                    None => tip.hide(),
                                }
                            }
                            ui.invalidate(id);
                        }
                        None
                    }
                    Event::MouseLeave | Event::CaptureChanged => {
                        if hover.get().is_some() || pressed.get().is_some() {
                            hover.set(None);
                            pressed.set(None);
                            if let Some(tip) = &tooltip {
                                tip.hide();
                            }
                            ui.invalidate(id);
                        }
                        None
                    }
                    Event::MouseDown {
                        x,
                        button: MouseButton::Left,
                        ..
                    } => {
                        pressed.set(item_at(bounds, *x, items.borrow().len()));
                        ui.invalidate(id);
                        None
                    }
                    Event::MouseUp {
                        x,
                        button: MouseButton::Left,
                        ..
                    } => {
                        let index = item_at(bounds, *x, items.borrow().len());
                        let was_pressed = pressed.get();
                        pressed.set(None);
                        ui.invalidate(id);
                        if index.is_none() || index != was_pressed {
                            return None;
                        }
                        let index = index?;
                        activated.set(index as i64);
                        let mapper = on_click.borrow();
                        mapper.as_ref().and_then(|mapper| mapper(index))
                    }
                    Event::KeyDown {
                        key,
                        repeat,
                        system,
                        ..
                    } if *repeat <= 1 && !*system => {
                        let count = items.borrow().len();
                        if count == 0 {
                            return None;
                        }
                        match *key {
                            Key::LEFT => {
                                let index = hover.get().unwrap_or(0).saturating_sub(1);
                                hover.set(Some(index));
                                ui.invalidate(id);
                                None
                            }
                            Key::RIGHT => {
                                let index = (hover.get().unwrap_or(0) + 1).min(count - 1);
                                hover.set(Some(index));
                                ui.invalidate(id);
                                None
                            }
                            Key::RETURN => {
                                let index = hover.get()?;
                                activated.set(index as i64);
                                ui.invalidate(id);
                                let mapper = on_click.borrow();
                                mapper.as_ref().and_then(|mapper| mapper(index))
                            }
                            _ => None,
                        }
                    }
                    _ => None,
                }
            });
        }

        Ok(Toolbar {
            control,
            items,
            hover,
            enabled,
            activated,
            _tooltip: tooltip,
            on_click,
        })
    }

    /// Appends an icon item with a hover tooltip.
    pub fn item(self, icon: impl Into<IconRef>, tooltip: &str) -> Toolbar<M> {
        self.push(Item {
            icon: Some(icon.into()),
            label: None,
            tooltip: (!tooltip.is_empty()).then(|| tooltip.to_string()),
        })
    }

    /// Appends an icon item with a hover tooltip and a text label after the
    /// icon.
    pub fn item_with_text(self, icon: impl Into<IconRef>, tooltip: &str, text: &str) -> Toolbar<M> {
        self.push(Item {
            icon: Some(icon.into()),
            label: Some(text.to_string()),
            tooltip: (!tooltip.is_empty()).then(|| tooltip.to_string()),
        })
    }

    /// Appends a prepared item and repaints.
    fn push(self, item: Item) -> Toolbar<M> {
        self.items.borrow_mut().push(item);
        self.control.invalidate();
        self
    }

    /// Maps a click to the app's message: the closure receives the item index
    /// and returns `Some(msg)` to raise it, or `None` to ignore the click.
    pub fn on_click(self, mapper: impl Fn(usize) -> Option<M> + 'static) -> Toolbar<M> {
        *self.on_click.borrow_mut() = Some(Box::new(mapper));
        self
    }

    /// The number of items.
    pub fn len(&self) -> usize {
        self.items.borrow().len()
    }

    /// Whether the toolbar has no items.
    pub fn is_empty(&self) -> bool {
        self.items.borrow().is_empty()
    }

    /// Item `index`'s icon, if it has one.
    pub fn icon(&self, index: usize) -> Option<IconRef> {
        self.items.borrow().get(index).and_then(|item| item.icon)
    }

    /// Item `index`'s label, if it has one.
    pub fn label(&self, index: usize) -> Option<String> {
        self.items
            .borrow()
            .get(index)
            .and_then(|item| item.label.clone())
    }

    /// Item `index`'s tooltip text, if it names one.
    pub fn tooltip(&self, index: usize) -> Option<String> {
        self.items
            .borrow()
            .get(index)
            .and_then(|item| item.tooltip.clone())
    }

    /// The toolbar's node identity.
    pub fn id(&self) -> WidgetId {
        self.control.id()
    }

    /// Enables or disables the toolbar. A disabled toolbar is dimmed and
    /// ignores input.
    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.set(enabled);
        self.control.set_enabled(enabled);
        self.control.invalidate();
    }

    /// Whether the toolbar is enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled.get()
    }

    /// Marks the toolbar selected, so its painter draws an outline (a form
    /// editor's selection).
    pub fn set_selected(&self, selected: bool) {
        self.control.set_selected(selected);
    }
}

impl<M: 'static> Properties for Toolbar<M> {
    fn properties(&self) -> Vec<Property> {
        vec![Property {
            name: "selected",
            value: Value::Integer(self.activated.get()),
        }]
    }

    fn set_property(&self, name: &str, value: Value) -> bool {
        match (name, value) {
            ("selected", Value::Integer(index)) => {
                let in_range = index >= 0 && (index as usize) < self.len();
                let index = if in_range { index } else { -1 };
                self.activated.set(index);
                self.hover.set((index >= 0).then_some(index as usize));
                self.control.invalidate();
                true
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests;
