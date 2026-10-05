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

mod layout;
mod paint;
mod place;
mod strip;

use layout::{Layout, Mode};
use strip::{Entry, Item, State};

/// Maps the index of a clicked item to an optional app message.
type ClickMapper<M> = Rc<RefCell<Option<Box<dyn Fn(usize) -> Option<M>>>>>;

/// The strip's current layout, measured with the backend's text metrics.
///
/// The entries borrow ends before this returns, and measuring text delivers no
/// events, so no borrow is held across a call that could re-enter the toolbar.
fn layout_of<M: 'static>(ui: &Ui<M>, id: WidgetId, state: &State) -> Layout {
    let bounds = ui.bounds(id);
    let dpi = ui.dpi();
    let style = layout::label_style(ui.theme_handle().get().text);
    let entries = state.entries.borrow();
    layout::compute(
        &entries,
        state.mode.get(),
        (bounds.width(), bounds.height()),
        dpi,
        &mut |text| ui.measure_text(text, &style, dpi).width,
    )
}

/// The hover tooltip text of item `index`, if it names one.
fn tooltip_at(state: &State, index: Option<usize>) -> Option<String> {
    let entries = state.entries.borrow();
    index
        .and_then(|index| entries.item(index))
        .and_then(|item| item.tooltip.clone())
}

/// A horizontal strip of clickable items, packed from the left.
///
/// An item is a text label, an icon, or an icon with a label, and may name a
/// tooltip. By default each button is only as wide as its content: an
/// icon-only button is square (as wide as the strip is tall), a labelled one is
/// its icon, gap, measured label and padding. The space right of the last item
/// stays empty. [`Toolbar::fill`] restores an equal split of the whole width.
///
/// [`Toolbar::separator`] adds a thin non-clickable line between groups. It
/// takes room but no index: `on_click`, [`Toolbar::icon`], [`Toolbar::label`],
/// [`Toolbar::tooltip`] and [`Toolbar::len`] count items only.
///
/// Items that do not fit are clipped at the right edge: a trailing item that
/// does not fit completely is neither drawn nor clickable. An overflow menu is
/// not supported yet.
///
/// Clicking an item maps its index to the app's `Msg` through
/// [`Toolbar::on_click`]. The left and right arrow keys move a focused item
/// (shown with the hover highlight) and Return activates it; a disabled
/// toolbar ignores input and dims its items.
pub struct Toolbar<M: 'static> {
    control: Control<M>,
    state: Rc<State>,
    activated: Rc<Cell<i64>>,
    _tooltip: Option<Rc<Tooltip<M>>>,
    on_click: ClickMapper<M>,
}

impl<M: 'static> Toolbar<M> {
    /// Creates a text-only toolbar of `items` along `bounds`, each button as
    /// wide as its label plus padding.
    ///
    /// Add icon items to an empty toolbar with [`Toolbar::empty`] and
    /// [`Toolbar::item`].
    pub(crate) fn new(ui: &Ui<M>, bounds: Rect, items: &[&str]) -> Result<Toolbar<M>> {
        let toolbar = Toolbar::empty(ui, bounds)?;
        {
            let mut slot = toolbar.state.entries.borrow_mut();
            for item in items {
                slot.push(Entry::Item(Item::text(item)));
            }
        }
        toolbar.control.invalidate();
        Ok(toolbar)
    }

    /// Creates an empty toolbar along `bounds`; add items with [`Toolbar::item`]
    /// and [`Toolbar::item_with_text`].
    pub(crate) fn empty(ui: &Ui<M>, bounds: Rect) -> Result<Toolbar<M>> {
        let control = Control::new(ui, &NodeSpec::new(NodeKind::Toolbar, bounds).tab_stop())?;
        let state = Rc::new(State::new());
        let activated = Rc::new(Cell::new(-1i64));
        let on_click: ClickMapper<M> = Rc::new(RefCell::new(None));

        // One hidden tip for the whole strip; its text follows the hovered item.
        // A toolbar without tooltips simply never shows it.
        let tooltip = Tooltip::attach(ui, control.id(), "").ok().map(Rc::new);

        {
            let state = Rc::clone(&state);
            let theme = ui.theme_handle();
            let selected = control.selected_handle();
            control.set_painter(Rc::new(move |canvas| {
                paint::paint(canvas, &state, &selected, theme.get());
            }));
        }

        {
            let state = Rc::clone(&state);
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
                if !state.enabled.get() {
                    return None;
                }
                match event {
                    Event::MouseMove { x, .. } => {
                        let index = layout_of(&ui, id, &state).item_at(*x);
                        if state.hover.get() != index {
                            state.hover.set(index);
                            if let Some(tip) = &tooltip {
                                match tooltip_at(&state, index) {
                                    Some(text) => tip.set_text(&text),
                                    None => tip.hide(),
                                }
                            }
                            ui.invalidate(id);
                        }
                        None
                    }
                    Event::MouseLeave | Event::CaptureChanged => {
                        if state.hover.get().is_some() || state.pressed.get().is_some() {
                            state.hover.set(None);
                            state.pressed.set(None);
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
                        state.pressed.set(layout_of(&ui, id, &state).item_at(*x));
                        ui.invalidate(id);
                        None
                    }
                    Event::MouseUp {
                        x,
                        button: MouseButton::Left,
                        ..
                    } => {
                        let index = layout_of(&ui, id, &state).item_at(*x);
                        let was_pressed = state.pressed.get();
                        state.pressed.set(None);
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
                        // Only items that fit are reachable: clipped items have
                        // empty spans and trail the visible ones, as for the mouse.
                        let count = layout_of(&ui, id, &state)
                            .items
                            .iter()
                            .take_while(|&&(start, end)| end > start)
                            .count();
                        if count == 0 {
                            return None;
                        }
                        match *key {
                            Key::LEFT => {
                                let index = state
                                    .hover
                                    .get()
                                    .unwrap_or(0)
                                    .min(count - 1)
                                    .saturating_sub(1);
                                state.hover.set(Some(index));
                                ui.invalidate(id);
                                None
                            }
                            Key::RIGHT => {
                                let index = (state.hover.get().unwrap_or(0) + 1).min(count - 1);
                                state.hover.set(Some(index));
                                ui.invalidate(id);
                                None
                            }
                            Key::RETURN => {
                                // A hover left over from a wider strip may now be clipped.
                                let index = state.hover.get().filter(|&index| index < count)?;
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
            state,
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

    /// Appends a thin vertical separator line after the last entry.
    ///
    /// It is not clickable, has no tooltip and takes no item index, so the
    /// indices `on_click` receives are unchanged by it.
    pub fn separator(self) -> Toolbar<M> {
        self.push_entry(Entry::Separator)
    }

    /// Splits the strip's whole width equally between the items instead of
    /// sizing each to its content (the layout before the compact default).
    /// Separators are drawn on the boundary before the next item.
    pub fn fill(self) -> Toolbar<M> {
        self.state.mode.set(Mode::Fill);
        self.control.invalidate();
        self
    }

    /// Appends a prepared item and repaints.
    fn push(self, item: Item) -> Toolbar<M> {
        self.push_entry(Entry::Item(item))
    }

    /// Appends an entry and repaints.
    fn push_entry(self, entry: Entry) -> Toolbar<M> {
        self.state.entries.borrow_mut().push(entry);
        self.control.invalidate();
        self
    }

    /// Maps a click to the app's message: the closure receives the item index
    /// and returns `Some(msg)` to raise it, or `None` to ignore the click.
    pub fn on_click(self, mapper: impl Fn(usize) -> Option<M> + 'static) -> Toolbar<M> {
        *self.on_click.borrow_mut() = Some(Box::new(mapper));
        self
    }

    /// The number of items; separators are not counted.
    pub fn len(&self) -> usize {
        self.state.entries.borrow().item_count()
    }

    /// Whether the toolbar has no items.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Item `index`'s icon, if it has one.
    pub fn icon(&self, index: usize) -> Option<IconRef> {
        self.state
            .entries
            .borrow()
            .item(index)
            .and_then(|item| item.icon)
    }

    /// Item `index`'s label, if it has one.
    pub fn label(&self, index: usize) -> Option<String> {
        self.state
            .entries
            .borrow()
            .item(index)
            .and_then(|item| item.label.clone())
    }

    /// Item `index`'s tooltip text, if it names one.
    pub fn tooltip(&self, index: usize) -> Option<String> {
        tooltip_at(&self.state, Some(index))
    }

    /// The toolbar's node identity.
    pub fn id(&self) -> WidgetId {
        self.control.id()
    }

    /// Enables or disables the toolbar. A disabled toolbar is dimmed and
    /// ignores input.
    pub fn set_enabled(&self, enabled: bool) {
        self.state.enabled.set(enabled);
        self.control.set_enabled(enabled);
        self.control.invalidate();
    }

    /// Whether the toolbar is enabled.
    pub fn is_enabled(&self) -> bool {
        self.state.enabled.get()
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
                self.state.hover.set((index >= 0).then_some(index as usize));
                self.control.invalidate();
                true
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod layout_tests;
#[cfg(test)]
mod tests;
