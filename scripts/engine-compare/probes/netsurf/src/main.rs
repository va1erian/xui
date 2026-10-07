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
    state: Rc<Cell<common::Load>>,
}

impl App for Probe {
    type Msg = Msg;
    fn update(&mut self, _: Msg, _: &mut Ui<Msg>) {
        self.view.update();
        self.state.set(if self.view.has_failed() {
            common::Load::Failed
        } else if self.view.is_ready() {
            common::Load::Ready
        } else {
            common::Load::Loading
        });
    }
}

fn main() {
    let args = common::args();
    let url = common::file_url(&args.page);
    let state = Rc::new(Cell::new(common::Load::Loading));
    let flag = Rc::clone(&state);
    common::run(
        "netsurf",
        &args,
        move |ui| {
            let view = NetSurfView::new(ui, ui.client_rect(), &url, || Msg)?;
            Ok(Probe { view, state: flag })
        },
        Msg,
        state,
    );
}
