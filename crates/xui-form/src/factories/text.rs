#![forbid(unsafe_code)]

//! Factories for the text widgets: labels, buttons, links, check boxes,
//! toggle buttons and text fields.

use xui_core::WidgetId;
use xui_core::arrange::{
    Handle, button, checkbox, edit, hyperlink, label, multiline_edit, toggle_button,
};
use xui_core::widget::{
    Button, CheckBox, Edit, HasText, Hyperlink, Label, MultilineEdit, Placeable, ToggleButton,
};

use super::{factory, id_of};
use crate::build::{BuildCx, Created, Factories, SetError, WidgetProps};
use crate::value::Value;

/// Registers the text widgets' factories.
pub(super) fn register<M: 'static>(factories: &mut Factories<M>) {
    factories.register(LabelFactory);
    factories.register(ButtonFactory);
    factories.register(HyperlinkFactory);
    factories.register(CheckBoxFactory);
    factories.register(ToggleButtonFactory);
    factories.register(EditFactory);
    factories.register(MultilineEditFactory);
}

factory!(LabelFactory, "Label", create_label);
factory!(ButtonFactory, "Button", create_button);
factory!(HyperlinkFactory, "Hyperlink", create_hyperlink);
factory!(CheckBoxFactory, "CheckBox", create_checkbox);
factory!(ToggleButtonFactory, "ToggleButton", create_toggle);
factory!(EditFactory, "Edit", create_edit);
factory!(MultilineEditFactory, "MultilineEdit", create_multiline);

fn create_label<M: 'static>(cx: &mut BuildCx<'_, M>) -> Created<M> {
    let handle = Handle::new();
    let entry = label(cx.text("text")).bind(&handle);
    cx.live(entry, Text::<Label<M>>::new(handle, None))
}

fn create_button<M: 'static>(cx: &mut BuildCx<'_, M>) -> Created<M> {
    let handle = Handle::new();
    let mut entry = button(cx.text("text")).bind(&handle);
    if let Some(handler) = cx.handler("Click") {
        entry = entry.on_click_with(move || handler(&[]));
    }
    cx.live(entry, Text::new(handle, Some(Button::set_enabled)))
}

fn create_hyperlink<M: 'static>(cx: &mut BuildCx<'_, M>) -> Created<M> {
    let handle = Handle::new();
    let mut entry = hyperlink(cx.text("text")).bind(&handle);
    if let Some(handler) = cx.handler("Click") {
        entry = entry.then(move |link| link.on_click(move || handler(&[])));
    }
    cx.live(entry, Text::new(handle, Some(Hyperlink::set_enabled)))
}

fn create_checkbox<M: 'static>(cx: &mut BuildCx<'_, M>) -> Created<M> {
    let handle = Handle::new();
    let mut entry = checkbox(cx.text("text"))
        .checked(cx.bool("checked", false))
        .bind(&handle);
    if let Some(handler) = cx.handler("Toggle") {
        entry = entry.then(move |check| check.on_toggle(move |on| handler(&[Value::Bool(on)])));
    }
    cx.live(entry, Checked(handle))
}

fn create_toggle<M: 'static>(cx: &mut BuildCx<'_, M>) -> Created<M> {
    let handle = Handle::new();
    let mut entry = toggle_button(cx.text("text"))
        .checked(cx.bool("checked", false))
        .bind(&handle);
    if let Some(handler) = cx.handler("Toggle") {
        entry = entry.then(move |toggle| toggle.on_toggle(move |on| handler(&[Value::Bool(on)])));
    }
    cx.live(entry, Checked(handle))
}

fn create_edit<M: 'static>(cx: &mut BuildCx<'_, M>) -> Created<M> {
    let handle = Handle::new();
    let cue = cx.text("cue");
    let mut entry = edit().text(cx.text("text")).bind(&handle);
    if !cue.is_empty() {
        entry = entry.placeholder(cue.clone());
    }
    if let Some(handler) = cx.handler("Change") {
        entry = entry
            .then(move |edit| edit.on_change(move |text| handler(&[Value::Text(text.to_owned())])));
    }
    cx.live(entry, EditProps { handle, cue })
}

fn create_multiline<M: 'static>(cx: &mut BuildCx<'_, M>) -> Created<M> {
    let handle = Handle::new();
    let text = cx.text("text");
    let mut entry = multiline_edit().bind(&handle).then(move |edit| {
        edit.set_text(&text);
        edit
    });
    if let Some(handler) = cx.handler("Change") {
        entry = entry
            .then(move |edit| edit.on_change(move |text| handler(&[Value::Text(text.to_owned())])));
    }
    cx.live(entry, Text::new(handle, Some(MultilineEdit::set_enabled)))
}

/// Reads and writes `text` on a widget with [`HasText`].
pub(super) fn text_prop<W: HasText>(widget: &W, prop: &str, value: &Value) -> Result<(), SetError> {
    match (prop, value) {
        ("text", Value::Text(text)) => {
            widget.set_text(text);
            Ok(())
        }
        ("text", _) => Err(SetError::TypeMismatch),
        _ => Err(SetError::UnknownProperty),
    }
}

/// A widget whose one own property is `text`.
pub(super) struct Text<W: 'static> {
    handle: Handle<W>,
    enable: Option<fn(&W, bool)>,
}

impl<W: 'static> Text<W> {
    /// The surface of the widget behind `handle`; `enable` dims it.
    pub(super) fn new(handle: Handle<W>, enable: Option<fn(&W, bool)>) -> Text<W> {
        Text { handle, enable }
    }
}

impl<M: 'static, W: HasText + Placeable<M> + 'static> WidgetProps<M> for Text<W> {
    fn id(&self) -> WidgetId {
        id_of(&self.handle)
    }

    fn get_own(&self, prop: &str) -> Option<Value> {
        (prop == "text").then(|| Value::Text(self.handle.get().text()))
    }

    fn set_own(&self, prop: &str, value: &Value) -> Result<(), SetError> {
        text_prop(&*self.handle.get(), prop, value)
    }

    fn set_enabled_hint(&self, enabled: bool) {
        if let Some(enable) = self.enable {
            enable(&self.handle.get(), enabled);
        }
    }
}

/// A check box or a toggle button: `text` and `checked`.
struct Checked<W: 'static>(Handle<W>);

/// The checked state both latching widgets share.
trait Latch {
    fn checked(&self) -> bool;
    fn set_checked(&self, checked: bool);
    fn set_enabled(&self, enabled: bool);
}

impl<M: 'static> Latch for CheckBox<M> {
    fn checked(&self) -> bool {
        self.is_checked()
    }
    fn set_checked(&self, checked: bool) {
        CheckBox::set_checked(self, checked);
    }
    fn set_enabled(&self, enabled: bool) {
        CheckBox::set_enabled(self, enabled);
    }
}

impl<M: 'static> Latch for ToggleButton<M> {
    fn checked(&self) -> bool {
        self.is_checked()
    }
    fn set_checked(&self, checked: bool) {
        ToggleButton::set_checked(self, checked);
    }
    fn set_enabled(&self, enabled: bool) {
        ToggleButton::set_enabled(self, enabled);
    }
}

impl<M: 'static, W: Latch + HasText + Placeable<M> + 'static> WidgetProps<M> for Checked<W> {
    fn id(&self) -> WidgetId {
        id_of(&self.0)
    }

    fn get_own(&self, prop: &str) -> Option<Value> {
        let widget = self.0.get();
        match prop {
            "text" => Some(Value::Text(widget.text())),
            "checked" => Some(Value::Bool(widget.checked())),
            _ => None,
        }
    }

    fn set_own(&self, prop: &str, value: &Value) -> Result<(), SetError> {
        match (prop, value) {
            ("checked", Value::Bool(checked)) => {
                self.0.get().set_checked(*checked);
                Ok(())
            }
            ("checked", _) => Err(SetError::TypeMismatch),
            _ => text_prop(&*self.0.get(), prop, value),
        }
    }

    fn set_enabled_hint(&self, enabled: bool) {
        self.0.get().set_enabled(enabled);
    }
}

/// An `Edit`: `text`, and the construction-only `cue`.
struct EditProps<M: 'static> {
    handle: Handle<Edit<M>>,
    cue: String,
}

impl<M: 'static> WidgetProps<M> for EditProps<M> {
    fn id(&self) -> WidgetId {
        id_of(&self.handle)
    }

    fn get_own(&self, prop: &str) -> Option<Value> {
        match prop {
            "text" => Some(Value::Text(self.handle.get().text())),
            "cue" => Some(Value::Text(self.cue.clone())),
            _ => None,
        }
    }

    fn set_own(&self, prop: &str, value: &Value) -> Result<(), SetError> {
        match prop {
            "cue" => Err(SetError::ReadOnly),
            _ => text_prop(&*self.handle.get(), prop, value),
        }
    }
}
