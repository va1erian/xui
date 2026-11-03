//! litehtml (`xui_litehtml::HtmlView`), given the page's HTML as a string as
//! the mail and help readers give it: no style sheet or local image loading.

#[path = "../../common.rs"]
mod common;

use std::cell::Cell;
use std::rc::Rc;

use xui_core::app::{App, Ui};
use xui_litehtml::HtmlView;

#[derive(Clone)]
struct Msg;

struct Probe {
    view: HtmlView<Msg>,
    state: Rc<Cell<common::Load>>,
}

impl App for Probe {
    type Msg = Msg;
    fn update(&mut self, _: Msg, _: &mut Ui<Msg>) {
        self.view.invalidate();
        if self.view.is_ready() {
            self.state.set(common::Load::Ready);
        }
    }
}

fn main() {
    let args = common::args();
    let html = std::fs::read_to_string(&args.page).expect("read the page");
    let state = Rc::new(Cell::new(common::Load::Loading));
    let flag = Rc::clone(&state);
    common::run(
        "litehtml",
        &args,
        move |ui| {
            let view = HtmlView::new(ui, ui.client_rect(), html, || Msg, |_| None)?;
            Ok(Probe { view, state: flag })
        },
        Msg,
        state,
    );
}
