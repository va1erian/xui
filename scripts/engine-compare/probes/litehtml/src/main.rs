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
    ready: Rc<Cell<bool>>,
}

impl App for Probe {
    type Msg = Msg;
    fn update(&mut self, _: Msg, _: &mut Ui<Msg>) {
        self.view.invalidate();
        self.ready.set(self.view.is_ready());
    }
}

fn main() {
    let args = common::args();
    let html = std::fs::read_to_string(&args.page).expect("read the page");
    let ready = Rc::new(Cell::new(false));
    let flag = Rc::clone(&ready);
    common::run(
        "litehtml",
        &args,
        move |ui| {
            let view = HtmlView::new(ui, ui.client_rect(), html, || Msg, |_| None)?;
            Ok(Probe { view, ready: flag })
        },
        Msg,
        ready,
    );
}
