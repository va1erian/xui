//! An `HtmlView` in an `xui_core::arrange` layout: it fills the slot it is
//! given, so the app never places it by hand.

use std::cell::Cell;
use std::rc::Rc;

use xui_canvas::OffscreenBackend;
use xui_core::app::{App, Ui, run_app};
use xui_core::arrange::{Handle, LayoutExt, build, column};
use xui_core::backend::{Backend, PlatformSpec};
use xui_core::geometry::Rect;
use xui_core::units::Dip;
use xui_litehtml::HtmlView;

struct Idle;

impl App for Idle {
    type Msg = u32;

    fn update(&mut self, _msg: u32, _ui: &mut Ui<u32>) {}
}

#[test]
fn a_layout_places_the_view_in_its_slot() {
    let backend: Rc<dyn Backend> = Rc::new(OffscreenBackend::new());
    let spec = PlatformSpec::new("litehtml place").size(Dip(400.0), Dip(300.0));
    let placed: Rc<Cell<Option<Rect>>> = Rc::new(Cell::new(None));
    let out = Rc::clone(&placed);
    run_app(backend, spec, move |ui| {
        let view: Handle<HtmlView<u32>> = Handle::new();
        ui.root(
            column().padding(40).child(
                build(|ui| {
                    HtmlView::new(ui, Rect::default(), "<p>Hello".to_string(), || 0, |_| None)
                })
                .bind(&view)
                .fill(1),
            ),
        )
        .expect("the layout mounts");
        out.set(Some(ui.bounds(view.get().id())));
        Idle
    })
    .expect("run_app succeeds");
    assert_eq!(
        placed.get(),
        Some(Rect::new(40, 40, 360, 260)),
        "the layout gives the view the padded slot"
    );
}
