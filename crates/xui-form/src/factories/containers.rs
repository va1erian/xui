#![forbid(unsafe_code)]

//! Factories for the containers and the separator: group boxes, panels and
//! divider lines.

use xui_core::WidgetId;
use xui_core::arrange::{Handle, group, panel, separator, vertical_separator};
use xui_core::widget::{GroupBox, HasText, Panel, Separator};

use super::text::text_prop;
use super::{factory, id_of};
use crate::build::{BuildCx, Created, Factories, SetError, WidgetProps};
use crate::value::Value;

/// Registers the container and separator factories.
pub(super) fn register<M: 'static>(factories: &mut Factories<M>) {
    factories.register(GroupBoxFactory);
    factories.register(PanelFactory);
    factories.register(SeparatorFactory);
}

factory!(GroupBoxFactory, "GroupBox", create_group);
factory!(PanelFactory, "Panel", create_panel);
factory!(SeparatorFactory, "Separator", create_separator);

/// A titled frame; its children are placed inside the frame, below the title.
fn create_group<M: 'static>(cx: &mut BuildCx<'_, M>) -> Created<M> {
    let handle = Handle::new();
    let entry = group(cx.text("text"), cx.take_content()).bind(&handle);
    cx.live(entry, Group(handle))
}

/// A plain container: its children are placed from its top-left corner.
fn create_panel<M: 'static>(cx: &mut BuildCx<'_, M>) -> Created<M> {
    let handle = Handle::new();
    let entry = panel(cx.take_content()).plain().bind(&handle);
    cx.live(entry, PanelProps(handle))
}

fn create_separator<M: 'static>(cx: &mut BuildCx<'_, M>) -> Created<M> {
    let handle = Handle::new();
    let vertical = cx.text("orientation") == "vertical";
    let entry = if vertical {
        vertical_separator()
    } else {
        separator()
    }
    .bind(&handle);
    cx.live(entry, Rule { handle, vertical })
}

/// A `GroupBox`: its title is `text`.
struct Group<M: 'static>(Handle<GroupBox<M>>);

impl<M: 'static> WidgetProps<M> for Group<M> {
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

/// A `Panel`: no properties of its own.
struct PanelProps<M: 'static>(Handle<Panel<M>>);

impl<M: 'static> WidgetProps<M> for PanelProps<M> {
    fn id(&self) -> WidgetId {
        id_of(&self.0)
    }

    fn get_own(&self, _prop: &str) -> Option<Value> {
        None
    }

    fn set_own(&self, _prop: &str, _value: &Value) -> Result<(), SetError> {
        Err(SetError::UnknownProperty)
    }

    fn set_enabled_hint(&self, enabled: bool) {
        self.0.get().set_enabled(enabled);
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
            "vertical"
        } else {
            "horizontal"
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
