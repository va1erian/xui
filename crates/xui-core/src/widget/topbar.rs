#![forbid(unsafe_code)]

//! [`TopBar`]: a portable material top band of mixed items.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use super::control::Control;
use crate::app::Ui;
use crate::backend::{NodeKind, NodeSpec, Result, WidgetId};
use crate::geometry::Rect;

mod events;
mod icon;
mod items;
mod paint;

#[cfg(test)]
mod tests;

use events::{ChangeMapper, ClickMapper, Events, ToggleMapper};
use items::{Item, Kind};

/// An opaque handle to one [`TopBar`] item.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TopBarId(usize);

impl TopBarId {
    /// Creates an id. Distinct items must get distinct ids.
    pub const fn new(value: usize) -> TopBarId {
        TopBarId(value)
    }
}

/// The sentinel id of an anonymous [`spacer`](TopBar::spacer).
const SPACER: TopBarId = TopBarId(usize::MAX);

/// A dependency-free icon.
///
/// The set is deliberately small: the bar carries it without an image
/// dependency (see issue #35), drawing each shape with the portable
/// [`Canvas`] and any other short mark as a text glyph. A caller that needs a
/// richer set can pass text, e.g. `Glyph::Text("+")`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Glyph {
    /// Three stacked bars (a menu).
    Menu,
    /// A magnifier (search).
    Search,
    /// An X (close).
    Close,
    /// Three dots (more).
    More,
    /// A five-pointed star (a favourite).
    Star,
    /// A short text run drawn as the icon, e.g. `"+"` or `"A"`.
    Text(&'static str),
}

/// A horizontal material band of icon buttons, toggles, labels, sliders and
/// spacers.
///
/// Each interactive item is addressed by an opaque [`TopBarId`] and maps its
/// event to the app's `Msg` through a closure given at construction:
/// [`on_click`](TopBar::on_click) for icon buttons,
/// [`on_toggle`](TopBar::on_toggle) for toggles and
/// [`on_change`](TopBar::on_change) for sliders. Spacers have no id and simply
/// share the leftover width, so trailing items are pushed to the edge.
pub struct TopBar<M: 'static> {
    control: Control<M>,
    items: Rc<RefCell<Vec<Item>>>,
    on_click: ClickMapper<M>,
    on_toggle: ToggleMapper<M>,
    on_change: ChangeMapper<M>,
}

impl<M: 'static> TopBar<M> {
    /// Creates an empty top bar along `bounds`; add items with the builders.
    pub fn new(ui: &Ui<M>, bounds: Rect) -> Result<TopBar<M>> {
        let control = Control::new(ui, &NodeSpec::new(NodeKind::Toolbar, bounds))?;
        let items = Rc::new(RefCell::new(Vec::new()));
        let hover = Rc::new(Cell::new(None));
        let active = Rc::new(Cell::new(None));
        let on_click: ClickMapper<M> = Rc::new(RefCell::new(None));
        let on_toggle: ToggleMapper<M> = Rc::new(RefCell::new(None));
        let on_change: ChangeMapper<M> = Rc::new(RefCell::new(None));

        {
            let items = Rc::clone(&items);
            let hover = Rc::clone(&hover);
            let active = Rc::clone(&active);
            let theme = ui.theme_handle();
            let selected = control.selected_handle();
            control.set_painter(Rc::new(move |canvas| {
                paint::paint(canvas, &items, &hover, &active, &selected, theme.get());
            }));
        }

        {
            let events = Events {
                ui: ui.clone(),
                id: control.id(),
                items: Rc::clone(&items),
                hover: Rc::clone(&hover),
                active: Rc::clone(&active),
                on_click: Rc::clone(&on_click),
                on_toggle: Rc::clone(&on_toggle),
                on_change: Rc::clone(&on_change),
            };
            control.on_events(move |event| events.handle(event));
        }

        Ok(TopBar {
            control,
            items,
            on_click,
            on_toggle,
            on_change,
        })
    }

    /// Appends an icon button.
    pub fn icon(self, id: TopBarId, glyph: Glyph) -> TopBar<M> {
        self.push(id, Kind::Icon(glyph))
    }

    /// Appends a toggle, starting unchecked.
    pub fn toggle(self, id: TopBarId, glyph: Glyph) -> TopBar<M> {
        self.push(
            id,
            Kind::Toggle {
                glyph,
                checked: false,
            },
        )
    }

    /// Appends a text label.
    pub fn label(self, id: TopBarId, text: &str) -> TopBar<M> {
        self.push(id, Kind::Label(text.to_string()))
    }

    /// Appends a slider over `min..=max`, starting at the minimum.
    pub fn slider(self, id: TopBarId, min: f64, max: f64) -> TopBar<M> {
        let (min, max) = if min <= max { (min, max) } else { (max, min) };
        self.push(
            id,
            Kind::Slider {
                min,
                max,
                value: min,
            },
        )
    }

    /// Appends an anonymous spacer that shares leftover width equally.
    pub fn spacer(self) -> TopBar<M> {
        self.spacer_weight(1)
    }

    /// Appends an anonymous spacer that shares leftover width by `weight`.
    pub fn spacer_weight(self, weight: u32) -> TopBar<M> {
        self.push(SPACER, Kind::Spacer(weight.max(1)))
    }

    /// Sets an item's hover tooltip.
    pub fn tooltip(self, id: TopBarId, text: &str) -> TopBar<M> {
        if let Some(item) = self
            .items
            .borrow_mut()
            .iter_mut()
            .find(|item| item.id == id)
        {
            item.tooltip = Some(text.to_string());
        }
        self
    }

    /// Maps an icon-button click to the app's message.
    pub fn on_click(self, mapper: impl Fn(TopBarId) -> Option<M> + 'static) -> TopBar<M> {
        *self.on_click.borrow_mut() = Some(Box::new(mapper));
        self
    }

    /// Maps a toggle to the app's message, carrying its new checked state.
    pub fn on_toggle(self, mapper: impl Fn(TopBarId, bool) -> Option<M> + 'static) -> TopBar<M> {
        *self.on_toggle.borrow_mut() = Some(Box::new(mapper));
        self
    }

    /// Maps a slider's new value to the app's message, while dragging.
    pub fn on_change(self, mapper: impl Fn(TopBarId, f64) -> Option<M> + 'static) -> TopBar<M> {
        *self.on_change.borrow_mut() = Some(Box::new(mapper));
        self
    }

    fn push(self, id: TopBarId, kind: Kind) -> TopBar<M> {
        self.items.borrow_mut().push(Item {
            id,
            enabled: true,
            tooltip: None,
            kind,
        });
        self.control.invalidate();
        self
    }

    /// The bar's node identity.
    pub fn id(&self) -> WidgetId {
        self.control.id()
    }

    /// The number of items.
    pub fn len(&self) -> usize {
        self.items.borrow().len()
    }

    /// Whether the bar has no items.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Calls `f` with the item `id`, if it exists.
    fn with<T>(&self, id: TopBarId, f: impl FnOnce(&Item) -> T) -> Option<T> {
        let items = self.items.borrow();
        items.iter().find(|item| item.id == id).map(f)
    }

    /// Enables or disables item `id`. A disabled item is dimmed and ignores
    /// input.
    pub fn set_enabled(&self, id: TopBarId, enabled: bool) {
        if let Some(item) = self
            .items
            .borrow_mut()
            .iter_mut()
            .find(|item| item.id == id)
        {
            item.enabled = enabled;
        }
        self.control.invalidate();
    }

    /// Whether item `id` is enabled.
    pub fn is_enabled(&self, id: TopBarId) -> bool {
        self.with(id, |item| item.enabled).unwrap_or(false)
    }

    /// Sets a toggle item's checked state without raising an event.
    pub fn set_checked(&self, id: TopBarId, checked: bool) {
        if let Some(item) = self
            .items
            .borrow_mut()
            .iter_mut()
            .find(|item| item.id == id)
            && let Kind::Toggle { checked: slot, .. } = &mut item.kind
        {
            *slot = checked;
        }
        self.control.invalidate();
    }

    /// Whether a toggle item `id` is checked.
    pub fn is_checked(&self, id: TopBarId) -> bool {
        self.with(id, |item| {
            matches!(item.kind, Kind::Toggle { checked: true, .. })
        })
        .unwrap_or(false)
    }

    /// Replaces a label item's text.
    pub fn set_text(&self, id: TopBarId, text: &str) {
        if let Some(item) = self
            .items
            .borrow_mut()
            .iter_mut()
            .find(|item| item.id == id)
            && let Kind::Label(slot) = &mut item.kind
        {
            *slot = text.to_string();
        }
        self.control.invalidate();
    }

    /// A label item's text.
    pub fn text(&self, id: TopBarId) -> Option<String> {
        self.with(id, |item| match &item.kind {
            Kind::Label(text) => Some(text.clone()),
            _ => None,
        })
        .flatten()
    }

    /// Sets a slider item's value, clamped to its range, without an event.
    pub fn set_value(&self, id: TopBarId, value: f64) {
        if let Some(item) = self
            .items
            .borrow_mut()
            .iter_mut()
            .find(|item| item.id == id)
            && let Kind::Slider {
                min,
                max,
                value: slot,
                ..
            } = &mut item.kind
        {
            *slot = value.clamp(*min, *max);
        }
        self.control.invalidate();
    }

    /// A slider item's value.
    pub fn value(&self, id: TopBarId) -> Option<f64> {
        self.with(id, |item| match item.kind {
            Kind::Slider { value, .. } => Some(value),
            _ => None,
        })
        .flatten()
    }

    /// An item's tooltip text.
    pub fn tooltip_text(&self, id: TopBarId) -> Option<String> {
        self.with(id, |item| item.tooltip.clone()).flatten()
    }

    /// Marks the bar selected, so its painter draws an outline (a form
    /// editor's selection).
    pub fn set_selected(&self, selected: bool) {
        self.control.set_selected(selected);
    }
}
