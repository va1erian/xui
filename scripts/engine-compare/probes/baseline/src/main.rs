//! The skeleton every probe shares, with no HTML engine: a label giving the
//! page's size. The other probes' sizes are measured against this one.

#[path = "../../common.rs"]
mod common;

use std::cell::Cell;
use std::rc::Rc;

use xui_core::app::{App, Ui};
use xui_core::arrange::{column, label};

#[derive(Clone)]
struct Msg;

struct Probe;

impl App for Probe {
    type Msg = Msg;
    fn update(&mut self, _: Msg, _: &mut Ui<Msg>) {}
}

fn main() {
    let args = common::args();
    let text = std::fs::read_to_string(&args.page).unwrap_or_default();
    let ready = Rc::new(Cell::new(true));
    common::run(
        "baseline",
        &args,
        move |ui| {
            ui.root(column().child(label(format!("{} bytes", text.len()))))?;
            Ok(Probe)
        },
        Msg,
        ready,
    );
}
