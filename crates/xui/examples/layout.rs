//! A small form built from layouts and builders: no coordinates, no `ui`
//! argument per widget, and fields only for the widgets the app changes.
//! Resize the window and the form re-flows; toggle the check box and the hint
//! row appears and disappears without leaving a gap.
//!
//! Run with:
//!
//! ```text
//! cargo run -p xui --example layout
//! ```
//!
//! `XUI_DEMO_AUTOCLOSE_MS` makes it quit itself; `XUI_SNAPSHOT=<dir>` saves a
//! light and a dark screenshot instead of opening a window.

use xui::prelude::*;

#[derive(Clone)]
enum Msg {
    Name(String),
    Loud(bool),
    Volume(f64),
    Reset,
    Quit,
}

#[derive(Default)]
struct Form {
    greeting: Handle<Label<Msg>>,
    hint: Handle<Label<Msg>>,
    volume: Handle<ProgressBar<Msg>>,
    name: Handle<Edit<Msg>>,
    loud: bool,
}

impl Form {
    fn greet(&self, name: &str) {
        let name = if name.is_empty() { "world" } else { name };
        let greeting = if self.loud {
            format!("HELLO, {}!", name.to_uppercase())
        } else {
            format!("Hello, {name}")
        };
        self.greeting.get().set_text(&greeting);
    }
}

impl App for Form {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, ui: &mut Ui<Msg>) {
        match msg {
            Msg::Name(name) => self.greet(&name),
            Msg::Loud(loud) => {
                self.loud = loud;
                ui.set_visible(self.hint.get().id(), loud);
                self.greet(&self.name.get().text());
            }
            Msg::Volume(value) => self.volume.get().set_value(value as i32),
            Msg::Reset => {
                self.name.get().set_text("");
                self.greet("");
            }
            Msg::Quit => ui.quit(),
        }
    }
}

fn main() -> Result<()> {
    xui::app("Layout demo").size(420, 300).run(|ui| {
        let form = Form::default();
        ui.root(
            column().padding(16).gap(8).children((
                grid([Track::Auto, Track::Fill(1)]).gap(8).children((
                    label("Name").align(Align::Center),
                    edit()
                        .placeholder("world")
                        .bind(&form.name)
                        .on_change(Msg::Name),
                    label("Volume").align(Align::Center),
                    slider(0.0, 100.0).on_change(Msg::Volume),
                )),
                label("Hello, world").title().bind(&form.greeting),
                checkbox("Loud").on_toggle(Msg::Loud),
                label("Shouting is on").caption().bind(&form.hint),
                progress(100).bind(&form.volume),
                spacer(),
                row().gap(8).justify(Align::End).children((
                    button("Reset").on_click(Msg::Reset),
                    button("Quit").on_click(Msg::Quit),
                )),
            )),
        )?;
        ui.set_visible(form.hint.get().id(), false);
        Ok(form)
    })
}
