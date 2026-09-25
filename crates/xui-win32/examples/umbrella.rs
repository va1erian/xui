//! The smallest app written against the `xui` umbrella crate: a label and a
//! button, so it never names a backend. The Win32 backend supplies the
//! Common Controls v6 manifest this package's `build.rs` embeds.
//!
//! Run with:
//!
//! ```text
//! cargo run -p xui-win32 --example umbrella
//! ```
//!
//! `WIN32UI_DEMO_AUTOCLOSE_MS` makes it quit itself, for headless smoke runs.

#[cfg(windows)]
mod umbrella {
    use xui::prelude::*;

    enum Msg {
        Bump,
        Autoclose,
    }

    struct Counter {
        label: Label,
        count: i32,
    }

    impl App for Counter {
        type Msg = Msg;

        fn update(&mut self, msg: Msg, ui: &mut Ui<Msg>) {
            match msg {
                Msg::Bump => {
                    self.count += 1;
                    self.label.set_text(&format!("{} clicks", self.count));
                }
                Msg::Autoclose => ui.quit(),
            }
        }
    }

    pub(crate) fn main() {
        let _ = xui::run_app(WindowSpec::new("xui umbrella"), |ui| {
            if let Some(millis) = std::env::var("WIN32UI_DEMO_AUTOCLOSE_MS")
                .ok()
                .and_then(|value| value.parse::<u32>().ok())
            {
                let _ = ui.set_timer(millis);
                ui.on_timer(|_| Some(Msg::Autoclose));
            }
            let label = Label::new(ui, Rect::new(16, 16, 320, 44), "0 clicks").unwrap();
            let button = Button::new(ui, "Click me")
                .unwrap()
                .on_click(|| Some(Msg::Bump));
            ui.set_layout(xui::column![label, button]);
            Counter { label, count: 0 }
        });
    }
}

#[cfg(windows)]
fn main() {
    umbrella::main();
}

#[cfg(not(windows))]
fn main() {}
