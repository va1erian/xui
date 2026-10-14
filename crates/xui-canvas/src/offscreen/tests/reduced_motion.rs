//! The offscreen backend reports no reduced-motion preference, so a headless
//! render does not depend on the accessibility setting of the machine it runs
//! on.

use super::*;

#[test]
fn the_offscreen_backend_reports_no_reduced_motion() {
    assert!(!OffscreenBackend::new().prefers_reduced_motion());
}

#[test]
fn an_app_on_the_offscreen_backend_sees_no_reduced_motion() {
    struct Idle;

    impl App for Idle {
        type Msg = ();
        fn update(&mut self, _msg: (), _ui: &mut Ui<()>) {}
    }

    let backend: Rc<dyn Backend> = Rc::new(OffscreenBackend::new());
    let mut seen = None;
    let result = run_app(backend, PlatformSpec::new("motion"), |ui| {
        seen = Some(ui.prefers_reduced_motion());
        Idle
    });

    assert!(result.is_ok(), "the app ran");
    assert_eq!(seen, Some(false));
}
