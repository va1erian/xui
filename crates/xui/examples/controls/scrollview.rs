//! Demonstrates the portable [`ScrollView`] container: ten rows created through
//! `scroll.ui()` are registered with `add`, and scrolling updates a label.
//!
//! Run with:
//!
//! ```text
//! cargo run -p xui --features canvas --example control_scrollview
//! ```

use xui_core::Dip;
use xui_core::app::{App, Ui, run_app};
use xui_core::backend::PlatformSpec;
use xui_core::widget::{HasText, Label, ScrollView};

#[path = "support.rs"]
mod support;
use support::{Layout, autoclose, backend};

enum Msg {
    Scroll(i32),
    Quit,
}

struct Demo {
    result: Label<Msg>,
    _rows: Vec<Label<Msg>>,
    _scroll: ScrollView<Msg>,
}

impl App for Demo {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, ui: &mut Ui<Msg>) {
        match msg {
            Msg::Scroll(offset) => self.result.set_text(&format!("Scroll offset: {offset}px")),
            Msg::Quit => ui.quit(),
        }
    }
}

fn main() -> xui_core::backend::Result<()> {
    run_app(
        backend(),
        PlatformSpec::new("ScrollView demo").size(Dip(520.0), Dip(360.0)),
        |ui| {
            let l = Layout::new(ui.dpi());
            let scroll = ScrollView::new(ui, l.rect(16.0, 16.0, 504.0, 240.0)).unwrap();
            let mut rows = Vec::new();
            for row in 0..10 {
                let label = Label::new(
                    scroll.ui(),
                    l.rect(0.0, 0.0, 10.0, 10.0),
                    &format!("Row {row}"),
                )
                .unwrap();
                scroll.add(label.id(), Dip(32.0));
                rows.push(label);
            }
            let scroll = scroll.on_scroll(|offset| Some(Msg::Scroll(offset.value())));
            let result =
                Label::new(ui, l.rect(16.0, 256.0, 504.0, 288.0), "Scroll offset: 0px").unwrap();
            autoclose(ui, || Msg::Quit);
            Demo {
                result,
                _rows: rows,
                _scroll: scroll,
            }
        },
    )
}
