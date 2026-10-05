#![forbid(unsafe_code)]

//! Factories for the widgets that choose among items: radio groups, combo
//! boxes and list views.

use std::cell::RefCell;

use xui_core::WidgetId;
use xui_core::arrange::{Handle, combo_box, list, radio_group};
use xui_core::widget::{ComboBox, ListView, RadioGroup};

use super::{factory, id_of};
use crate::build::{BuildCx, Created, Factories, SetError, WidgetProps};
use crate::value::Value;

/// Registers the choice widgets' factories.
pub(super) fn register<M: 'static>(factories: &mut Factories<M>) {
    factories.register(RadioGroupFactory);
    factories.register(ComboBoxFactory);
    factories.register(ListViewFactory);
}

factory!(RadioGroupFactory, "RadioGroup", create_radio);
factory!(ComboBoxFactory, "ComboBox", create_combo);
factory!(ListViewFactory, "ListView", create_list);

/// The items as the `&str` slice the builders take.
fn labels(items: &[String]) -> Vec<&str> {
    items.iter().map(String::as_str).collect()
}

/// The non-negative `selected` property, if any.
fn initial(cx: &BuildCx<'_, impl Sized>, default: i64) -> Option<usize> {
    usize::try_from(cx.int("selected", default)).ok()
}

fn create_radio<M: 'static>(cx: &mut BuildCx<'_, M>) -> Created<M> {
    let handle = Handle::new();
    let items = cx.list("items");
    let mut entry = radio_group(&labels(&items)).bind(&handle);
    if let Some(index) = initial(cx, 0) {
        entry = entry.selected(index);
    }
    if let Some(handler) = cx.handler("Select") {
        entry = entry
            .then(move |group| group.on_select(move |index| handler(&[Value::Int(index as i64)])));
    }
    cx.live(entry, Radio { handle, items })
}

fn create_combo<M: 'static>(cx: &mut BuildCx<'_, M>) -> Created<M> {
    let handle = Handle::new();
    let items = cx.list("items");
    let mut entry = combo_box(&labels(&items)).bind(&handle);
    if let Some(index) = initial(cx, 0) {
        entry = entry.then(move |combo| {
            combo.select(index);
            combo
        });
    }
    if let Some(handler) = cx.handler("Select") {
        entry = entry
            .then(move |combo| combo.on_select(move |index| handler(&[Value::Int(index as i64)])));
    }
    cx.live(entry, Combo { handle, items })
}

fn create_list<M: 'static>(cx: &mut BuildCx<'_, M>) -> Created<M> {
    let handle = Handle::new();
    let items = cx.list("items");
    let multi = cx.bool("multi_select", false);
    // The schema default is -1 (no selection), and the writer drops
    // defaults, so a missing key means "nothing selected".
    let selected = initial(cx, -1);
    let mut entry = list()
        .items(&labels(&items))
        .bind(&handle)
        .then(move |list| {
            list.select(selected);
            list.multi_select(multi)
        });
    if let Some(handler) = cx.handler("Select") {
        entry =
            entry.then(move |list| list.on_select(move |row| handler(&[Value::Int(row as i64)])));
    }
    if let Some(handler) = cx.handler("Activate") {
        entry =
            entry.then(move |list| list.on_activate(move |row| handler(&[Value::Int(row as i64)])));
    }
    cx.live(
        entry,
        List {
            handle,
            items: RefCell::new(items),
            multi,
        },
    )
}

/// The error for writing the construction-only `items`, or a mistyped or
/// unknown property.
fn other(prop: &str) -> SetError {
    match prop {
        "items" | "selected" => SetError::TypeMismatch,
        _ => SetError::UnknownProperty,
    }
}

/// A `RadioGroup`: one node per option.
struct Radio<M: 'static> {
    handle: Handle<RadioGroup<M>>,
    items: Vec<String>,
}

impl<M: 'static> WidgetProps<M> for Radio<M> {
    fn id(&self) -> WidgetId {
        id_of(&self.handle)
    }

    fn get_own(&self, prop: &str) -> Option<Value> {
        match prop {
            "items" => Some(Value::List(self.items.clone())),
            "selected" => Some(Value::Int(self.handle.get().selected() as i64)),
            _ => None,
        }
    }

    fn set_own(&self, prop: &str, value: &Value) -> Result<(), SetError> {
        match (prop, value) {
            ("selected", Value::Int(index)) if *index >= 0 => {
                self.handle.get().select(*index as usize);
                Ok(())
            }
            ("items", _) => Err(SetError::ReadOnly),
            _ => Err(other(prop)),
        }
    }

    fn set_enabled_hint(&self, enabled: bool) {
        self.handle.get().set_enabled(enabled);
    }

    fn node_ids(&self) -> Vec<WidgetId> {
        self.handle.get().ids()
    }
}

/// A `ComboBox`: construction-only `items`, and `selected`.
struct Combo<M: 'static> {
    handle: Handle<ComboBox<M>>,
    items: Vec<String>,
}

impl<M: 'static> WidgetProps<M> for Combo<M> {
    fn id(&self) -> WidgetId {
        id_of(&self.handle)
    }

    fn get_own(&self, prop: &str) -> Option<Value> {
        match prop {
            "items" => Some(Value::List(self.items.clone())),
            "selected" => Some(Value::Int(self.handle.get().selected() as i64)),
            _ => None,
        }
    }

    fn set_own(&self, prop: &str, value: &Value) -> Result<(), SetError> {
        match (prop, value) {
            ("selected", Value::Int(index)) if *index >= 0 => {
                self.handle.get().select(*index as usize);
                Ok(())
            }
            ("items", _) => Err(SetError::ReadOnly),
            _ => Err(other(prop)),
        }
    }

    fn set_enabled_hint(&self, enabled: bool) {
        self.handle.get().set_enabled(enabled);
    }
}

/// A `ListView`: replaceable `items`, `selected` (or -1), and the
/// construction-only `multi_select`.
struct List<M: 'static> {
    handle: Handle<ListView<M>>,
    items: RefCell<Vec<String>>,
    multi: bool,
}

impl<M: 'static> WidgetProps<M> for List<M> {
    fn id(&self) -> WidgetId {
        id_of(&self.handle)
    }

    fn get_own(&self, prop: &str) -> Option<Value> {
        match prop {
            "items" => Some(Value::List(self.items.borrow().clone())),
            "selected" => Some(Value::Int(
                self.handle.get().selected().map_or(-1, |row| row as i64),
            )),
            "multi_select" => Some(Value::Bool(self.multi)),
            _ => None,
        }
    }

    fn set_own(&self, prop: &str, value: &Value) -> Result<(), SetError> {
        match (prop, value) {
            ("selected", Value::Int(index)) => {
                self.handle.get().select(usize::try_from(*index).ok());
                Ok(())
            }
            ("items", Value::List(items)) => {
                self.handle.get().set_items(&labels(items));
                *self.items.borrow_mut() = items.clone();
                Ok(())
            }
            ("multi_select", _) => Err(SetError::ReadOnly),
            _ => Err(other(prop)),
        }
    }

    fn set_enabled_hint(&self, enabled: bool) {
        self.handle.get().set_enabled(enabled);
    }
}
