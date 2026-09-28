//! A small form built with `xui_core::arrange`: no coordinates and no
//! per-widget fields. Resize the window and the form re-flows; toggle the
//! check box and the hint row appears and disappears without leaving a gap.
//!
//! Run with:
//!
//! ```text
//! XUI_BACKEND=canvas cargo run -p xui --features canvas --example layout
//! ```
//!
//! `XUI_DEMO_AUTOCLOSE_MS` makes it quit itself.

use std::rc::Rc;

use xui_core::app::{App, Ui, run_app};
use xui_core::arrange::{LayoutExt, Mounted, column, row, spacer};
use xui_core::backend::{PlatformSpec, Result};
use xui_core::widget::{Button, CheckBox, Edit, HasText, Label, ProgressBar, Slider};
use xui_core::{Dip, Insets, dip};

// This demo needs only `backend` and `autoclose`; the DIP converter is for the
// hand-placed demos.
#[allow(dead_code)]
#[path = "controls/support.rs"]
mod support;
use support::{autoclose, backend};

enum Msg {
    Name(String),
    Loud(bool),
    Volume(f64),
    Reset,
    Quit,
}

struct Form {
    greeting: Rc<Label<Msg>>,
    hint: Rc<Label<Msg>>,
    volume: Rc<ProgressBar<Msg>>,
    name: Rc<Edit<Msg>>,
    loud: bool,
    /// The whole widget tree: one field instead of one per widget.
    _mounted: Mounted<Msg>,
}

impl Form {
    fn greet(&self, name: &str) {
        let name = if name.is_empty() { "world" } else { name };
        let greeting = if self.loud {
            format!("HELLO, {}!", name.to_uppercase())
        } else {
            format!("Hello, {name}")
        };
        self.greeting.set_text(&greeting);
    }
}

impl App for Form {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, ui: &mut Ui<Msg>) {
        match msg {
            Msg::Name(name) => self.greet(&name),
            Msg::Loud(loud) => {
                self.loud = loud;
                ui.set_visible(self.hint.id(), loud);
                self.greet(&self.name.text());
            }
            Msg::Volume(value) => self.volume.set_value(value as i32),
            Msg::Reset => {
                self.name.set_text("");
                self.greet("");
            }
            Msg::Quit => ui.quit(),
        }
        // A longer greeting changes the label's natural size.
        ui.relayout();
    }
}

/// Builds the form. Constructor results go straight into the layout; the only
/// `?` is for the widgets the app keeps a handle to.
fn build(ui: &Ui<Msg>) -> Result<Form> {
    let greeting = Rc::new(Label::auto(ui, "Hello, world")?);
    let hint = Rc::new(Label::auto(ui, "Shouting is on")?);
    let volume = Rc::new(ProgressBar::auto(ui, 100)?);
    let name = Rc::new(Edit::auto(ui, "")?.on_change(|text| Some(Msg::Name(text.to_string()))));
    ui.set_visible(hint.id(), false);

    let root = column()
        .margins(Insets::all(dip(16.0)))
        .spacing(dip(8.0))
        .child(&greeting)
        .child(&name)
        .child(
            CheckBox::auto(ui, "Loud")
                .map(|check| check.on_toggle(|checked| Some(Msg::Loud(checked)))),
        )
        .child(&hint)
        .child(
            Slider::auto(ui, 0.0, 100.0).map(|slider| slider.on_change(|v| Some(Msg::Volume(v)))),
        )
        .child(&volume)
        .child(spacer())
        .child(
            row()
                .spacing(dip(8.0))
                .child(spacer())
                .child(Button::auto(ui, "Reset").map(|b| b.on_click(|| Some(Msg::Reset))))
                .child(Button::auto(ui, "Quit").map(|b| b.on_click(|| Some(Msg::Quit))))
                .height(dip(28.0)),
        );

    Ok(Form {
        greeting,
        hint,
        volume,
        name,
        loud: false,
        _mounted: ui.mount(root)?,
    })
}

fn main() -> Result<()> {
    run_app(
        backend(),
        PlatformSpec::new("Layout demo").size(Dip(420.0), Dip(280.0)),
        |ui| {
            autoclose(ui, || Msg::Quit);
            build(ui).expect("the form's widgets were created")
        },
    )
}
