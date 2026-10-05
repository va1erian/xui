//! Regression test for #160: `Slider`'s groove must render, not just its
//! thumb.
//!
//! The painter (`xui_core::widget::slider::Slider::new`) drew the unfilled
//! portion of the track with `theme.scrollbar_track`, a token that is defined
//! to equal `theme.background` (it's meant to make a real scrollbar's own,
//! separately-affording track blend into the page). Reused for a slider's
//! groove, painted directly over the plain background, that made the
//! unfilled part of the track invisible; at a low value, with little or no
//! filled (accent) portion either, only the thumb showed. The fix gave
//! sliders and progress bars their own `theme.track` token, guaranteed
//! distinct from the background (`theme::tokens::the_slider_track_stays_visible_on_the_background`).
//! This test renders a real, offscreen `Slider` and checks actual pixels: the
//! unfilled groove beyond the thumb must differ from the surrounding
//! background.

use std::cell::RefCell;
use std::rc::Rc;

use xui_canvas::{OffscreenBackend, RgbaImage};
use xui_core::Dip;
use xui_core::app::{App, Ui, run_app};
use xui_core::backend::{Backend, PlatformSpec};
use xui_core::geometry::Rect;
use xui_core::prelude::{LayoutExt, absolute, slider};

struct Demo;

impl App for Demo {
    type Msg = ();
    fn update(&mut self, _msg: (), _ui: &mut Ui<()>) {}
}

/// Renders a single `Slider` at `bounds`, set to `value`, and returns the
/// offscreen frame.
fn render_slider(bounds: Rect, value: f64) -> RgbaImage {
    let backend = Rc::new(OffscreenBackend::new());
    let rendered: Rc<RefCell<Option<RgbaImage>>> = Rc::new(RefCell::new(None));
    let backend_for_make = Rc::clone(&backend);
    let rendered_for_make = Rc::clone(&rendered);
    let run: Rc<dyn Backend> = backend.clone();

    run_app(
        run,
        PlatformSpec::new("slider track regression").size(Dip(200.0), Dip(80.0)),
        move |ui| {
            let slider = slider(0.0, 100.0).then(move |slider| {
                slider.set_value(value);
                slider
            });
            ui.root(absolute().child(slider.at(
                bounds.left,
                bounds.top,
                bounds.width(),
                bounds.height(),
            )))
            .unwrap();
            let image = backend_for_make.render(ui.window()).expect("a render");
            *rendered_for_make.borrow_mut() = Some(image);
            Demo
        },
    )
    .expect("the offscreen slider ran");

    rendered.borrow_mut().take().expect("a rendered frame")
}

#[test]
fn the_slider_groove_is_visible_beyond_the_thumb() {
    let bounds = Rect::new(16, 16, 184, 44);
    // Near the minimum, so the accent-filled portion is negligible and only
    // the groove (not the thumb) can show a low-value track at all.
    let image = render_slider(bounds, 1.0);

    let background = xui_core::Theme::light().background;
    let mid_y = ((bounds.top + bounds.bottom) / 2) as u32;

    // Well past the thumb, still short of the right inset: must be the
    // groove colour, not the plain background.
    let mut saw_track_pixel = false;
    for x in 40..170u32 {
        let [r, g, b, _] = image.pixel(x, mid_y).expect("in bounds");
        if (r, g, b) != (background.r, background.g, background.b) {
            saw_track_pixel = true;
            break;
        }
    }
    assert!(
        saw_track_pixel,
        "the slider's groove must paint something distinct from the background \
         along its unfilled length, not just the thumb"
    );
}
