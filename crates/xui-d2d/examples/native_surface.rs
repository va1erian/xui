//! A node with a native child window of its own, drawn without the software
//! composite: here an animated gradient written with `softbuffer`, where an app
//! would hand the window to wgpu on its render thread.
//!
//! Run with:
//!
//! ```text
//! cargo run -p xui-d2d --example native_surface
//! ```
//!
//! Setting `XUI_DEMO_AUTOCLOSE_MS` makes it quit itself.

#[cfg(windows)]
mod demo {
    use std::cell::{Cell, RefCell};
    use std::num::NonZeroU32;
    use std::rc::Rc;

    use xui_core::app::{App, run_app};
    use xui_core::backend::{Backend, NativeSurface, NodeKind, NodeSpec, PlatformSpec};
    use xui_core::widget::Control;
    use xui_core::{Color, Dip, Rect};
    use xui_d2d::D2dBackend;

    /// What the node draws into while it has a window.
    type Target = softbuffer::Surface<NativeSurface, NativeSurface>;

    struct Demo {
        _view: Control<()>,
    }

    impl App for Demo {
        type Msg = ();

        fn update(&mut self, _msg: (), _ui: &mut xui_core::app::Ui<()>) {}
    }

    /// Fills `target`, `width` by `height`, with a gradient that moves with `t`.
    fn draw(target: &mut Target, (width, height): (u32, u32), t: u32) {
        let (Some(w), Some(h)) = (NonZeroU32::new(width), NonZeroU32::new(height)) else {
            return;
        };
        if target.resize(w, h).is_err() {
            return;
        }
        let Ok(mut buffer) = target.buffer_mut() else {
            return;
        };
        for (i, pixel) in buffer.iter_mut().enumerate() {
            let (x, y) = (i as u32 % width, i as u32 / width);
            let red = (x + t) % 256;
            let green = (y + t / 2) % 256;
            *pixel = (red << 16) | (green << 8) | 0x60;
        }
        let _ = buffer.present();
    }

    pub fn run() {
        let backend = Rc::new(D2dBackend::new());
        let dynamic: Rc<dyn Backend> = backend;
        let _ = run_app(
            dynamic,
            PlatformSpec::new("xui-d2d native surface").size(Dip(520.0), Dip(420.0)),
            move |ui| {
                let bounds = Rect::new(40, 40, 440, 340);
                let spec = NodeSpec::new(NodeKind::Custom, bounds).contained();
                let view = Control::new(ui, &spec).expect("the view node");
                view.set_painter(Rc::new(|canvas| {
                    canvas.clear(Color::rgb(40, 40, 48));
                }));
                let id = view.id();
                ui.set_native_surface(id, true);

                let target: RefCell<Option<Target>> = RefCell::new(None);
                let frame = Cell::new(0u32);
                let size = (bounds.width() as u32, bounds.height() as u32);
                let anim = ui.set_timer(16);
                let autoclose = std::env::var("XUI_DEMO_AUTOCLOSE_MS")
                    .ok()
                    .and_then(|millis| millis.parse::<u32>().ok())
                    .map(|millis| ui.set_timer(millis));
                let timer_ui = ui.clone();
                ui.on_timer(move |fired| {
                    if Some(fired) == autoclose {
                        timer_ui.quit();
                    } else if fired == anim {
                        let mut target = target.borrow_mut();
                        if target.is_none()
                            && let Some(window) = timer_ui.native_surface(id)
                            && let Ok(context) = softbuffer::Context::new(window.clone())
                        {
                            *target = softbuffer::Surface::new(&context, window).ok();
                        }
                        if let Some(target) = target.as_mut() {
                            frame.set(frame.get().wrapping_add(2));
                            draw(target, size, frame.get());
                        }
                    }
                    None
                });
                Demo { _view: view }
            },
        );
    }
}

fn main() {
    #[cfg(windows)]
    demo::run();
}
