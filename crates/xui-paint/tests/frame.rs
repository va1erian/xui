//! The canvas view's frame is stroked on the edge of its node, so it is two
//! half pixels: one inside the node and one outside. It must not be drawn
//! under a clip of the node's own rectangle, which would cut the outer one.

use xui_canvas::snapshot::{Snapshot, render_with};
use xui_core::app::{App, Ui};
use xui_core::arrange::{Handle, LayoutExt, absolute, build};
use xui_core::geometry::Rect;
use xui_core::{Dip, Theme};
use xui_paint::view::{Msg, PaintCanvas};

struct Host;

impl App for Host {
    type Msg = Msg;
    fn update(&mut self, _msg: Msg, _ui: &mut Ui<Msg>) {}
}

#[test]
fn the_frame_keeps_its_outer_half_on_every_side() {
    let view = Rect::new(20, 20, 180, 120);
    for theme in [Theme::light(), Theme::dark()] {
        let image = render_with(
            Snapshot::new(Dip(220.0), Dip(160.0)).theme(theme),
            move |ui| {
                let canvas = Handle::new();
                ui.root(absolute().child(build(PaintCanvas::new).bind(&canvas).at(
                    view.left,
                    view.top,
                    view.width(),
                    view.height(),
                )))?;
                Ok(Host)
            },
            |_| {},
        )
        .expect("the canvas renders");
        let at = |x: i32, y: i32| image.pixel(x as u32, y as u32).expect("inside the image");
        let (mx, my) = ((view.left + view.right) / 2, (view.top + view.bottom) / 2);
        // Each outer half pixel against the window two pixels further out.
        let sides = [
            ("top", at(mx, view.top - 1), at(mx, view.top - 3)),
            ("bottom", at(mx, view.bottom), at(mx, view.bottom + 2)),
            ("left", at(view.left - 1, my), at(view.left - 3, my)),
            ("right", at(view.right, my), at(view.right + 2, my)),
        ];
        for (side, frame, window) in sides {
            assert_ne!(frame, window, "the outer half of the {side} edge is drawn");
        }
    }
}
