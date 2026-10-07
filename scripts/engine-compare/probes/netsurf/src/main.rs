//! NetSurf (`xui_netsurf::NetSurfView`), opening the page's `file:` URL.

#[path = "../../common.rs"]
mod common;

use std::cell::Cell;
use std::rc::Rc;

use xui_core::app::{App, Ui};
use xui_netsurf::NetSurfView;

#[derive(Clone)]
struct Msg;

struct Probe {
    view: NetSurfView<Msg>,
    ready: Rc<Cell<bool>>,
}

impl App for Probe {
    type Msg = Msg;
    fn update(&mut self, _: Msg, _: &mut Ui<Msg>) {
        self.view.update();
        self.ready.set(self.view.is_ready() || self.view.has_failed());
    }
}

fn main() {
    let args = common::args();
    let url = common::file_url(&args.page);
    let ready = Rc::new(Cell::new(false));
    let flag = Rc::clone(&ready);
    common::run(
        "netsurf",
        &args,
        move |ui| {
            let view = NetSurfView::new(ui, ui.client_rect(), &url, || Msg)?;
            Ok(Probe { view, ready: flag })
        },
        Msg,
        ready,
    );
}
