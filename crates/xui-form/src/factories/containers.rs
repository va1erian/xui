#![forbid(unsafe_code)]

//! Factories for the container widgets and the separator: panels, group
//! boxes, tab controls and divider lines.

use xui_core::WidgetId;
use xui_core::arrange::{Handle, group, panel, separator, tabs, vertical_separator};
use xui_core::widget::{GroupBox, HasText, Panel, Separator, Tabs};

use super::text::text_prop;
use super::{factory, id_of};
use crate::build::{BuildCx, Created, Factories, SetError, WidgetProps};
use crate::value::Value;

/// Registers the container and separator factories.
pub(super) fn register<M: 'static>(factories: &mut Factories<M>) {
    factories.register(GroupFactory);
    factories.register(PanelFactory);
    factories.register(TabsFactory);
    factories.register(SeparatorFactory);
}

factory!(GroupFactory, "Group", create_group);
factory!(PanelFactory, "Panel", create_panel);
factory!(TabsFactory, "Tabs", create_tabs);
factory!(SeparatorFactory, "Separator", create_separator);

/// A titled frame; its content is laid out inside the frame, below the title.
fn create_group<M: 'static>(cx: &mut BuildCx<'_, M>) -> Created<M> {
    let handle = Handle::new();
    let entry = group(cx.text("text"), cx.take_content()).bind(&handle);
    cx.live(entry, GroupProps(handle))
}

/// A container, plain unless it asks for a card.
fn create_panel<M: 'static>(cx: &mut BuildCx<'_, M>) -> Created<M> {
    let handle = Handle::new();
    let card = cx.bool("card", false);
    let mut entry = panel(cx.take_content());
    if !card {
        entry = entry.plain();
    }
    cx.live(entry.bind(&handle), PanelProps { handle, card })
}

/// A tab control, one page per page of the node.
fn create_tabs<M: 'static>(cx: &mut BuildCx<'_, M>) -> Created<M> {
    let handle = Handle::new();
    let mut entry = tabs();
    for (title, content) in cx.take_pages() {
        entry = entry.page(title, content);
    }
    if let Some(handler) = cx.handler("Change") {
        entry = entry.on_change_with(move |index| handler(&[Value::Int(index as i64)]));
    }
    let selected = usize::try_from(cx.int("selected", 0)).unwrap_or_default();
    cx.live(entry.bind(&handle), TabsProps { handle, selected })
}

fn create_separator<M: 'static>(cx: &mut BuildCx<'_, M>) -> Created<M> {
    let handle = Handle::new();
    let vertical = cx.text("orientation") == "Vertical";
    let entry = if vertical {
        vertical_separator()
    } else {
        separator()
    }
    .bind(&handle);
    cx.live(entry, Rule { handle, vertical })
}

/// A `Group`: its title is `text`.
struct GroupProps<M: 'static>(Handle<GroupBox<M>>);

impl<M: 'static> WidgetProps<M> for GroupProps<M> {
    fn id(&self) -> WidgetId {
        id_of(&self.0)
    }

    fn get_own(&self, prop: &str) -> Option<Value> {
        (prop == "text").then(|| Value::Text(self.0.get().text()))
    }

    fn set_own(&self, prop: &str, value: &Value) -> Result<(), SetError> {
        text_prop(&*self.0.get(), prop, value)
    }
}

/// A `Panel`: only its construction-only `card`.
struct PanelProps<M: 'static> {
    handle: Handle<Panel<M>>,
    card: bool,
}

impl<M: 'static> WidgetProps<M> for PanelProps<M> {
    fn id(&self) -> WidgetId {
        id_of(&self.handle)
    }

    fn get_own(&self, prop: &str) -> Option<Value> {
        (prop == "card").then_some(Value::Bool(self.card))
    }

    fn set_own(&self, prop: &str, _value: &Value) -> Result<(), SetError> {
        match prop {
            "card" => Err(SetError::ReadOnly),
            _ => Err(SetError::UnknownProperty),
        }
    }

    fn set_enabled_hint(&self, enabled: bool) {
        self.handle.get().set_enabled(enabled);
    }
}

/// A `Tabs`: the `selected` page, first selected once it exists.
struct TabsProps<M: 'static> {
    handle: Handle<Tabs<M>>,
    selected: usize,
}

impl<M: 'static> WidgetProps<M> for TabsProps<M> {
    fn id(&self) -> WidgetId {
        id_of(&self.handle)
    }

    fn ready(&self) {
        self.handle.get().select(self.selected);
    }

    fn get_own(&self, prop: &str) -> Option<Value> {
        (prop == "selected").then(|| Value::Int(self.handle.get().selected() as i64))
    }

    fn set_own(&self, prop: &str, value: &Value) -> Result<(), SetError> {
        match (prop, value) {
            ("selected", Value::Int(index)) if *index >= 0 => {
                self.handle.get().select(*index as usize);
                Ok(())
            }
            ("selected", _) => Err(SetError::TypeMismatch),
            _ => Err(SetError::UnknownProperty),
        }
    }

    fn set_enabled_hint(&self, enabled: bool) {
        self.handle.get().set_enabled(enabled);
    }
}

/// A `Separator`: its construction-only `orientation`.
struct Rule<M: 'static> {
    handle: Handle<Separator<M>>,
    vertical: bool,
}

impl<M: 'static> WidgetProps<M> for Rule<M> {
    fn id(&self) -> WidgetId {
        id_of(&self.handle)
    }

    fn get_own(&self, prop: &str) -> Option<Value> {
        let orientation = if self.vertical {
            "Vertical"
        } else {
            "Horizontal"
        };
        (prop == "orientation").then(|| Value::Enum(orientation.to_owned()))
    }

    fn set_own(&self, prop: &str, _value: &Value) -> Result<(), SetError> {
        match prop {
            "orientation" => Err(SetError::ReadOnly),
            _ => Err(SetError::UnknownProperty),
        }
    }
}
