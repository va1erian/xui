#![forbid(unsafe_code)]

//! Checkable [`Toolbar`] items: the builders, the state accessors, the click
//! latch and the accessibility description.

use super::Toolbar;
use super::strip::{Item, State};
use crate::accessibility::{Node, Role};
use crate::icon::IconRef;

/// Flips item `index` if it is checkable: a click latches its state.
pub(super) fn flip(state: &State, index: usize) {
    if let Some(item) = state.entries.borrow_mut().item_mut(index)
        && let Some(checked) = item.checked
    {
        item.checked = Some(!checked);
    }
}

impl<M: 'static> Toolbar<M> {
    /// Appends a checkable icon item (an active tool, a visible layer),
    /// unchecked. A click flips its state before `on_click` runs; read it with
    /// [`Toolbar::is_checked`] and set it from `update` with
    /// [`Toolbar::set_checked`].
    pub fn toggle_item(self, icon: impl Into<IconRef>, tooltip: &str) -> Toolbar<M> {
        self.push(Item {
            checked: Some(false),
            ..Item::icon(icon.into(), None, tooltip)
        })
    }

    /// Appends a checkable icon item with a text label; see
    /// [`Toolbar::toggle_item`].
    pub fn toggle_item_with_text(
        self,
        icon: impl Into<IconRef>,
        tooltip: &str,
        text: &str,
    ) -> Toolbar<M> {
        self.push(Item {
            checked: Some(false),
            ..Item::icon(icon.into(), Some(text), tooltip)
        })
    }

    /// Whether item `index` is checked. `None` when it does not exist or is
    /// not checkable (see [`Toolbar::toggle_item`]).
    pub fn is_checked(&self, index: usize) -> Option<bool> {
        self.state
            .entries
            .borrow()
            .item(index)
            .and_then(|item| item.checked)
    }

    /// Checks or unchecks item `index` without raising `on_click`. Does
    /// nothing for an item that is missing or not checkable.
    pub fn set_checked(&self, index: usize, checked: bool) {
        let changed = {
            let mut entries = self.state.entries.borrow_mut();
            match entries.item_mut(index) {
                Some(item) if item.checked.is_some_and(|now| now != checked) => {
                    item.checked = Some(checked);
                    true
                }
                _ => false,
            }
        };
        if changed {
            self.control.invalidate();
        }
    }

    /// Describes the toolbar to assistive technology: a tool bar whose
    /// children are buttons, a checkable one carrying its checked state.
    pub fn accessibility(&self) -> Node {
        let entries = self.state.entries.borrow();
        let enabled = self.state.enabled.get();
        let items = entries.items().map(|item| {
            let name = item
                .label
                .as_deref()
                .or(item.tooltip.as_deref())
                .unwrap_or_default();
            let mut node = Node::new(Role::Button, name).invokable().enabled(enabled);
            if let Some(tip) = &item.tooltip {
                node = node.help(tip.as_str());
            }
            match item.checked {
                Some(checked) => node.checked(checked),
                None => node,
            }
        });
        Node::new(Role::ToolBar, "")
            .enabled(enabled)
            .children(items)
    }
}
