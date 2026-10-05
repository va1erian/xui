#![forbid(unsafe_code)]

//! Factories for the widgets over a numeric range: number fields, sliders
//! and progress bars.

use std::cell::Cell;

use xui_core::arrange::{Handle, number_field, progress, slider};
use xui_core::widget::{NumberField, ProgressBar, Slider};
use xui_core::{Properties, WidgetId};

use super::{factory, id_of};
use crate::build::{BuildCx, Created, Factories, SetError, WidgetProps};
use crate::value::Value;

/// Registers the range widgets' factories.
pub(super) fn register<M: 'static>(factories: &mut Factories<M>) {
    factories.register(NumberFieldFactory);
    factories.register(SliderFactory);
    factories.register(ProgressBarFactory);
}

factory!(NumberFieldFactory, "NumberField", create_number);
factory!(SliderFactory, "Slider", create_slider);
factory!(ProgressBarFactory, "ProgressBar", create_progress);

fn create_number<M: 'static>(cx: &mut BuildCx<'_, M>) -> Created<M> {
    let handle = Handle::new();
    let (min, max) = (cx.float("min", 0.0), cx.float("max", 100.0));
    let value = cx.float("value", min);
    let mut entry = number_field(min, max, cx.float("step", 1.0))
        .bind(&handle)
        .then(move |field| {
            field.set_value(value);
            field
        });
    if let Some(handler) = cx.handler("Change") {
        entry = entry.then(move |field| field.on_change(move |v| handler(&[Value::Float(v)])));
    }
    if let Some(handler) = cx.handler("Commit") {
        entry = entry.then(move |field| field.on_commit(move |v| handler(&[Value::Float(v)])));
    }
    cx.live(entry, Number::new(handle, min, max))
}

fn create_slider<M: 'static>(cx: &mut BuildCx<'_, M>) -> Created<M> {
    let handle = Handle::new();
    let (min, max) = (cx.float("min", 0.0), cx.float("max", 100.0));
    let value = cx.float("value", min);
    let mut entry = slider(min, max).bind(&handle).then(move |slider| {
        slider.set_value(value);
        slider
    });
    if let Some(handler) = cx.handler("Change") {
        entry = entry.then(move |slider| slider.on_change(move |v| handler(&[Value::Float(v)])));
    }
    if let Some(handler) = cx.handler("Commit") {
        entry = entry.then(move |slider| slider.on_commit(move |v| handler(&[Value::Float(v)])));
    }
    cx.live(entry, Number::new(handle, min, max))
}

fn create_progress<M: 'static>(cx: &mut BuildCx<'_, M>) -> Created<M> {
    let handle = Handle::new();
    let value = cx.int("value", 0) as i32;
    let entry = progress(cx.int("max", 100) as i32)
        .value(value)
        .bind(&handle);
    cx.live(entry, Progress(handle))
}

/// The value and range operations a number field and a slider share.
trait Ranged {
    fn value(&self) -> f64;
    fn set_value(&self, value: f64);
    fn set_range(&self, min: f64, max: f64);
    fn set_enabled(&self, enabled: bool);
}

impl<M: 'static> Ranged for NumberField<M> {
    fn value(&self) -> f64 {
        NumberField::value(self)
    }
    fn set_value(&self, value: f64) {
        NumberField::set_value(self, value);
    }
    fn set_range(&self, min: f64, max: f64) {
        self.set_property("min", xui_core::Value::Float(min));
        self.set_property("max", xui_core::Value::Float(max));
    }
    fn set_enabled(&self, enabled: bool) {
        NumberField::set_enabled(self, enabled);
    }
}

impl<M: 'static> Ranged for Slider<M> {
    fn value(&self) -> f64 {
        Slider::value(self)
    }
    fn set_value(&self, value: f64) {
        Slider::set_value(self, value);
    }
    fn set_range(&self, min: f64, max: f64) {
        Slider::set_range(self, min, max);
    }
    fn set_enabled(&self, enabled: bool) {
        Slider::set_enabled(self, enabled);
    }
}

/// A number field or a slider: `value`, `min` and `max`. An int is accepted
/// wherever a float is expected (see `ValueType`).
struct Number<W: 'static> {
    handle: Handle<W>,
    min: Cell<f64>,
    max: Cell<f64>,
}

impl<W: 'static> Number<W> {
    fn new(handle: Handle<W>, min: f64, max: f64) -> Number<W> {
        Number {
            handle,
            min: Cell::new(min),
            max: Cell::new(max),
        }
    }
}

impl<M: 'static, W> WidgetProps<M> for Number<W>
where
    W: Ranged + xui_core::widget::Placeable<M> + 'static,
{
    fn id(&self) -> WidgetId {
        id_of(&self.handle)
    }

    fn get_own(&self, prop: &str) -> Option<Value> {
        match prop {
            "value" => Some(Value::Float(self.handle.get().value())),
            "min" => Some(Value::Float(self.min.get())),
            "max" => Some(Value::Float(self.max.get())),
            _ => None,
        }
    }

    fn set_own(&self, prop: &str, value: &Value) -> Result<(), SetError> {
        if !matches!(prop, "value" | "min" | "max") {
            return Err(SetError::UnknownProperty);
        }
        let number = value.as_float().ok_or(SetError::TypeMismatch)?;
        let widget = self.handle.get();
        match prop {
            "value" => widget.set_value(number),
            "min" => {
                self.min.set(number);
                widget.set_range(number, self.max.get());
            }
            _ => {
                self.max.set(number);
                widget.set_range(self.min.get(), number);
            }
        }
        Ok(())
    }

    fn set_enabled_hint(&self, enabled: bool) {
        self.handle.get().set_enabled(enabled);
    }
}

/// A `ProgressBar`: `value` and `max`.
struct Progress<M: 'static>(Handle<ProgressBar<M>>);

impl<M: 'static> WidgetProps<M> for Progress<M> {
    fn id(&self) -> WidgetId {
        id_of(&self.0)
    }

    fn get_own(&self, prop: &str) -> Option<Value> {
        let bar = self.0.get();
        match prop {
            "value" => Some(Value::Int(bar.value() as i64)),
            "max" => Some(Value::Int(bar.max() as i64)),
            _ => None,
        }
    }

    fn set_own(&self, prop: &str, value: &Value) -> Result<(), SetError> {
        let bar = self.0.get();
        match (prop, value) {
            ("value", Value::Int(value)) => bar.set_value(*value as i32),
            ("max", Value::Int(value)) => bar.set_max(*value as i32),
            ("value" | "max", _) => return Err(SetError::TypeMismatch),
            _ => return Err(SetError::UnknownProperty),
        }
        Ok(())
    }

    fn set_enabled_hint(&self, enabled: bool) {
        self.0.get().set_enabled(enabled);
    }
}
