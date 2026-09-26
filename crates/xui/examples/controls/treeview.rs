//! Demonstrates the portable [`TreeView`] over a tiny in-memory tree: selecting
//! a node reports its id to a label, expanding a branch reports the toggle.
//!
//! Run with:
//!
//! ```text
//! cargo run -p xui --features canvas --example control_treeview
//! ```

use xui_core::Dip;
use xui_core::app::{App, Ui, run_app};
use xui_core::backend::PlatformSpec;
use xui_core::widget::{HasText, Label, TreeRow, TreeView};

#[path = "support.rs"]
mod support;
use support::{Layout, autoclose, backend};

enum Msg {
    Select(usize),
    Toggle(usize, bool),
    Quit,
}

struct Demo {
    result: Label<Msg>,
    _tree: TreeView<Msg>,
}

impl App for Demo {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, ui: &mut Ui<Msg>) {
        match msg {
            Msg::Select(id) => self.result.set_text(&format!("Selected node {id}")),
            Msg::Toggle(id, expanded) => {
                self.result.set_text(&format!(
                    "Node {id} {}",
                    if expanded { "expanded" } else { "collapsed" }
                ));
            }
            Msg::Quit => ui.quit(),
        }
    }
}

fn main() -> xui_core::backend::Result<()> {
    run_app(
        backend(),
        PlatformSpec::new("TreeView demo").size(Dip(520.0), Dip(360.0)),
        |ui| {
            let l = Layout::new(ui.dpi());
            let result =
                Label::new(ui, l.rect(16.0, 16.0, 504.0, 48.0), "Nothing selected").unwrap();
            let tree = TreeView::new(
                ui,
                l.rect(16.0, 64.0, 504.0, 240.0),
                &[
                    TreeRow::new("Inbox", 0).expandable(true).expanded(true),
                    TreeRow::new("Work", 1),
                    TreeRow::new("Archive", 0).expandable(true),
                ],
            )
            .unwrap()
            .on_select(|id| Some(Msg::Select(id)))
            .on_toggle(|id, expanded| Some(Msg::Toggle(id, expanded)));
            autoclose(ui, || Msg::Quit);
            Demo {
                result,
                _tree: tree,
            }
        },
    )
}
