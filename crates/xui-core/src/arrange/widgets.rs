#![forbid(unsafe_code)]

//! Builders for the portable widgets: `label("Name")`, `button("OK")`, …
//!
//! Each returns a [`Build`] whose methods set the widget's options and map
//! its events to the app's `Msg`. A value event takes the message's
//! constructor directly (`edit().on_change(Msg::Name)`); a plain event takes
//! the message (`button("OK").on_click(Msg::Ok)`), with an `_with` form for a
//! closure that may raise nothing.

use super::{Build, build};
use crate::icon::IconRef;
use crate::widget::{
    Button, CheckBox, ColumnWidth, ComboBox, Edit, HasText, Hyperlink, Label, ListView,
    MultilineEdit, NumberField, ProgressBar, Separator, Slider, StatusBar, ToggleButton,
};

/// A line of text.
pub fn label<M: 'static>(text: impl Into<String>) -> Build<Label<M>, M> {
    let text = text.into();
    build(move |ui| Label::auto(ui, &text))
}

impl<M: 'static> Build<Label<M>, M> {
    /// Styles the label as a heading.
    pub fn title(self) -> Build<Label<M>, M> {
        self.then(Label::title)
    }

    /// Styles the label as a secondary caption.
    pub fn caption(self) -> Build<Label<M>, M> {
        self.then(Label::caption)
    }
}

/// A push button.
pub fn button<M: 'static>(text: impl Into<String>) -> Build<Button<M>, M> {
    let text = text.into();
    build(move |ui| Button::auto(ui, &text))
}

impl<M: 'static> Build<Button<M>, M> {
    /// Raises `msg` on a click.
    pub fn on_click(self, msg: M) -> Build<Button<M>, M>
    where
        M: Clone,
    {
        self.then(move |button| button.on_click(move || Some(msg.clone())))
    }

    /// Maps a click through `f`, which may raise nothing.
    pub fn on_click_with(self, f: impl Fn() -> Option<M> + 'static) -> Build<Button<M>, M> {
        self.then(move |button| button.on_click(f))
    }

    /// Draws `icon` before the label (or alone, for an empty label).
    pub fn icon(self, icon: impl Into<IconRef>) -> Build<Button<M>, M> {
        let icon = icon.into();
        self.then(move |button| button.icon(icon))
    }

    /// Shows `text` in a tooltip while the pointer rests on the button.
    pub fn tooltip(self, text: impl Into<String>) -> Build<Button<M>, M> {
        let text = text.into();
        self.then_with(move |button, _| {
            button.set_tooltip(&text)?;
            Ok(button)
        })
    }

    /// Makes it the default action: an accent face.
    pub fn primary(self) -> Build<Button<M>, M> {
        self.then(Button::primary)
    }
}

/// A link-styled button.
pub fn hyperlink<M: 'static>(text: impl Into<String>) -> Build<Hyperlink<M>, M> {
    let text = text.into();
    build(move |ui| Hyperlink::auto(ui, &text))
}

impl<M: 'static> Build<Hyperlink<M>, M> {
    /// Raises `msg` on a click.
    pub fn on_click(self, msg: M) -> Build<Hyperlink<M>, M>
    where
        M: Clone,
    {
        self.then(move |link| link.on_click(move || Some(msg.clone())))
    }
}

/// A check box.
pub fn checkbox<M: 'static>(text: impl Into<String>) -> Build<CheckBox<M>, M> {
    let text = text.into();
    build(move |ui| CheckBox::auto(ui, &text))
}

impl<M: 'static> Build<CheckBox<M>, M> {
    /// Starts checked or not.
    pub fn checked(self, checked: bool) -> Build<CheckBox<M>, M> {
        self.then(move |check| {
            check.set_checked(checked);
            check
        })
    }

    /// Raises `f(checked)` when the user toggles it.
    pub fn on_toggle(self, f: impl Fn(bool) -> M + 'static) -> Build<CheckBox<M>, M> {
        self.then(move |check| check.on_toggle(move |on| Some(f(on))))
    }
}

/// A button that stays down.
pub fn toggle_button<M: 'static>(text: impl Into<String>) -> Build<ToggleButton<M>, M> {
    let text = text.into();
    build(move |ui| ToggleButton::auto(ui, &text))
}

impl<M: 'static> Build<ToggleButton<M>, M> {
    /// Starts down or not.
    pub fn checked(self, checked: bool) -> Build<ToggleButton<M>, M> {
        self.then(move |toggle| {
            toggle.set_checked(checked);
            toggle
        })
    }

    /// Raises `f(down)` when the user toggles it.
    pub fn on_toggle(self, f: impl Fn(bool) -> M + 'static) -> Build<ToggleButton<M>, M> {
        self.then(move |toggle| toggle.on_toggle(move |on| Some(f(on))))
    }

    /// Draws `icon` before the label (or alone, for an empty label).
    pub fn icon(self, icon: impl Into<IconRef>) -> Build<ToggleButton<M>, M> {
        let icon = icon.into();
        self.then(move |toggle| toggle.icon(icon))
    }

    /// Shows `text` in a tooltip while the pointer rests on the button.
    pub fn tooltip(self, text: impl Into<String>) -> Build<ToggleButton<M>, M> {
        let text = text.into();
        self.then_with(move |toggle, _| {
            toggle.set_tooltip(&text)?;
            Ok(toggle)
        })
    }
}

/// A single-line text field.
pub fn edit<M: 'static>() -> Build<Edit<M>, M> {
    build(|ui| Edit::auto(ui, ""))
}

impl<M: 'static> Build<Edit<M>, M> {
    /// Starts with `text`.
    pub fn text(self, text: impl Into<String>) -> Build<Edit<M>, M> {
        let text = text.into();
        self.then(move |edit| {
            edit.set_text(&text);
            edit
        })
    }

    /// Shows `cue` while the field is empty.
    pub fn placeholder(self, cue: impl Into<String>) -> Build<Edit<M>, M> {
        let cue = cue.into();
        self.then(move |edit| edit.cue(&cue))
    }

    /// Masks the text, for a password.
    pub fn password(self) -> Build<Edit<M>, M> {
        self.then(|edit| edit.password(true))
    }

    /// Raises `f(text)` whenever the text changes.
    pub fn on_change(self, f: impl Fn(String) -> M + 'static) -> Build<Edit<M>, M> {
        self.then(move |edit| edit.on_change(move |text| Some(f(text.to_string()))))
    }
}

/// A multi-line text field.
pub fn multiline_edit<M: 'static>() -> Build<MultilineEdit<M>, M> {
    build(|ui| MultilineEdit::auto(ui, ""))
}

impl<M: 'static> Build<MultilineEdit<M>, M> {
    /// Raises `f(text)` whenever the text changes.
    pub fn on_change(self, f: impl Fn(String) -> M + 'static) -> Build<MultilineEdit<M>, M> {
        self.then(move |edit| edit.on_change(move |text| Some(f(text.to_string()))))
    }
}

/// A numeric field from `min` to `max` in steps of `step`.
pub fn number_field<M: 'static>(min: f64, max: f64, step: f64) -> Build<NumberField<M>, M> {
    build(move |ui| NumberField::auto(ui, min, max, step))
}

impl<M: 'static> Build<NumberField<M>, M> {
    /// Raises `f(value)` whenever the value changes.
    pub fn on_change(self, f: impl Fn(f64) -> M + 'static) -> Build<NumberField<M>, M> {
        self.then(move |field| field.on_change(move |value| Some(f(value))))
    }
}

/// A slider from `min` to `max`.
pub fn slider<M: 'static>(min: f64, max: f64) -> Build<Slider<M>, M> {
    build(move |ui| Slider::auto(ui, min, max))
}

impl<M: 'static> Build<Slider<M>, M> {
    /// Raises `f(value)` while the thumb moves.
    pub fn on_change(self, f: impl Fn(f64) -> M + 'static) -> Build<Slider<M>, M> {
        self.then(move |slider| slider.on_change(move |value| Some(f(value))))
    }
}

/// A drop-down list of `items`.
pub fn combo_box<M: 'static>(items: &[&str]) -> Build<ComboBox<M>, M> {
    let items: Vec<String> = items.iter().map(|item| item.to_string()).collect();
    build(move |ui| {
        let items: Vec<&str> = items.iter().map(String::as_str).collect();
        ComboBox::auto(ui, &items)
    })
}

impl<M: 'static> Build<ComboBox<M>, M> {
    /// Raises `f(index)` when the user picks an item.
    pub fn on_select(self, f: impl Fn(usize) -> M + 'static) -> Build<ComboBox<M>, M> {
        self.then(move |combo| combo.on_select(move |index| Some(f(index))))
    }
}

/// A progress bar from zero to `max`.
pub fn progress<M: 'static>(max: i32) -> Build<ProgressBar<M>, M> {
    build(move |ui| ProgressBar::auto(ui, max))
}

impl<M: 'static> Build<ProgressBar<M>, M> {
    /// Starts at `value`.
    pub fn value(self, value: i32) -> Build<ProgressBar<M>, M> {
        self.then(move |bar| {
            bar.set_value(value);
            bar
        })
    }
}

/// A horizontal rule.
pub fn separator<M: 'static>() -> Build<Separator<M>, M> {
    build(Separator::auto)
}

/// A status bar with one part per entry of `parts`.
pub fn status_bar<M: 'static>(parts: &[&str]) -> Build<StatusBar<M>, M> {
    let parts: Vec<String> = parts.iter().map(|part| part.to_string()).collect();
    build(move |ui| {
        let parts: Vec<&str> = parts.iter().map(String::as_str).collect();
        StatusBar::auto(ui, &parts)
    })
}

/// A table or list of rows; add columns with [`column`](Build::column) and
/// fill it with [`ListView::set_model`] (or [`ListView::refresh_model`] for
/// live data).
pub fn list<M: 'static>() -> Build<ListView<M>, M> {
    build(|ui| ListView::auto(ui, Vec::<Vec<String>>::new()))
}

impl<M: 'static> Build<ListView<M>, M> {
    /// Adds a left-aligned column `width` design units wide (or
    /// [`Fill`](crate::widget::Fill) for the rest).
    pub fn column(
        self,
        title: impl Into<String>,
        width: impl Into<ColumnWidth>,
    ) -> Build<ListView<M>, M> {
        let (title, width) = (title.into(), width.into());
        self.then(move |list| list.column(title, width))
    }

    /// Adds a right-aligned column, for numbers.
    pub fn column_right(
        self,
        title: impl Into<String>,
        width: impl Into<ColumnWidth>,
    ) -> Build<ListView<M>, M> {
        let (title, width) = (title.into(), width.into());
        self.then(move |list| list.column_right(title, width))
    }

    /// Raises `f(row)` when the selection moves to `row`.
    pub fn on_select(self, f: impl Fn(usize) -> M + 'static) -> Build<ListView<M>, M> {
        self.then(move |list| list.on_select(move |row| Some(f(row))))
    }

    /// Raises `f(row)` when `row` is activated (double-click or Enter).
    pub fn on_activate(self, f: impl Fn(usize) -> M + 'static) -> Build<ListView<M>, M> {
        self.then(move |list| list.on_activate(move |row| Some(f(row))))
    }
}
