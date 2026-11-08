//! `on_painted`: one message per document, after its first paint.

use std::cell::RefCell;
use std::rc::Rc;

use xui_canvas::snapshot::{Snapshot, Stage, render_with};
use xui_core::app::{App, Ui};
use xui_core::{Dip, Rect, Theme};
use xui_markdown::MarkdownView;

struct Host {
    _view: Rc<MarkdownView<String>>,
    log: Rc<RefCell<Vec<String>>>,
}

impl App for Host {
    type Msg = String;

    fn update(&mut self, msg: String, _ui: &mut Ui<String>) {
        self.log.borrow_mut().push(msg);
    }
}

type Rig = (Rc<MarkdownView<String>>, Rc<RefCell<Vec<String>>>);

/// A view showing `# One` with `bounds`, and the messages the app heard.
fn run(bounds: Rect, body: impl FnOnce(&Stage<'_, String>, &Rig) + 'static) {
    let slot: Rc<RefCell<Option<Rig>>> = Rc::new(RefCell::new(None));
    let build_slot = Rc::clone(&slot);
    render_with(
        Snapshot::new(Dip(320.0), Dip(200.0)).theme(Theme::light()),
        move |ui| {
            let log = Rc::new(RefCell::new(Vec::new()));
            let view = MarkdownView::new(ui, bounds)?
                .markdown("# One")
                .on_painted(|| Some("painted".to_owned()));
            let view = Rc::new(view);
            *build_slot.borrow_mut() = Some((Rc::clone(&view), Rc::clone(&log)));
            Ok(Host { _view: view, log })
        },
        move |stage| {
            let rig = slot.borrow_mut().take().expect("built");
            body(stage, &rig);
        },
    )
    .expect("render");
}

/// Paints a frame and lets the app take what the paint queued.
fn paint(stage: &Stage<'_, String>) {
    stage.ui().capture().expect("capture");
    stage.emit("frame".to_owned());
}

#[test]
fn each_document_reports_its_first_paint_once() {
    run(Rect::new(0, 0, 320, 200), |stage, (view, log)| {
        paint(stage);
        paint(stage);
        assert_eq!(*log.borrow(), ["painted", "frame", "frame"]);
        log.borrow_mut().clear();
        view.set_markdown("# Two");
        stage.emit("set".to_owned());
        assert_eq!(*log.borrow(), ["set"], "nothing before the paint");
        paint(stage);
        paint(stage);
        assert_eq!(*log.borrow(), ["set", "painted", "frame", "frame"]);
    });
}

#[test]
fn a_view_with_no_room_waits_for_one() {
    run(Rect::new(0, 0, 0, 0), |stage, (view, log)| {
        paint(stage);
        assert_eq!(*log.borrow(), ["frame"]);
        view.set_bounds(Rect::new(0, 0, 320, 200));
        paint(stage);
        assert_eq!(*log.borrow(), ["frame", "painted", "frame"]);
    });
}
