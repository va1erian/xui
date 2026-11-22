//! The app's icon reaches its windows, and the windows it opens inherit it.

use super::*;
use crate::backend::WindowIcon;
use crate::image::Image;

struct Quiet;

impl App for Quiet {
    type Msg = ();

    fn update(&mut self, _msg: (), _ui: &mut Ui<()>) {}
}

fn picture(shade: u8) -> WindowIcon {
    WindowIcon::image(Image::from_rgba(2, 2, vec![shade; 16]).expect("image"))
}

/// Runs an app with `icon` on a headless backend and hands its `Ui` to
/// `check` while the window is still open.
fn launch_with(icon: Option<WindowIcon>, check: impl FnOnce(&HeadlessBackend, &Ui<()>)) {
    let backend = Rc::new(HeadlessBackend::new());
    let mut launch = crate::app("icon").backend(backend.clone());
    if let Some(icon) = icon {
        launch = launch.icon(icon);
    }
    let mut check = Some(check);
    launch
        .run(|ui| {
            if let Some(check) = check.take() {
                check(&backend, ui);
            }
            Ok(Quiet)
        })
        .expect("the app ran");
    assert!(check.is_none(), "the app was built");
}

#[test]
fn the_launch_icon_is_set_on_the_main_window() {
    launch_with(Some(WindowIcon::resource(1)), |backend, ui| {
        assert_eq!(
            backend.window_icon(ui.window()),
            Some(WindowIcon::Resource(1))
        );
    });
}

#[test]
fn without_an_icon_the_backend_default_is_left_alone() {
    launch_with(None, |backend, ui| {
        assert_eq!(backend.window_icon(ui.window()), None);
        let child = ui
            .open_window(PlatformSpec::new("child"), |_ui| Quiet)
            .expect("child");
        assert_eq!(backend.window_icon(child.window()), None);
        child.close();
    });
}

#[test]
fn secondary_and_modal_windows_inherit_the_app_icon() {
    launch_with(Some(picture(1)), |backend, ui| {
        let child = ui
            .open_window(PlatformSpec::new("child"), |_ui| Quiet)
            .expect("child");
        assert_eq!(backend.window_icon(child.window()), Some(picture(1)));
        child.close();

        let seen = Cell::new(None);
        let _: Option<()> = ui.open_modal(PlatformSpec::new("modal"), |child_ui| {
            seen.set(Some(
                backend.window_icon(child_ui.window()) == Some(picture(1)),
            ));
            child_ui.close();
            Quiet
        });
        assert_eq!(seen.get(), Some(true), "the modal child inherited the icon");
    });
}

#[test]
fn a_windows_own_icon_wins_over_the_inherited_one() {
    launch_with(Some(picture(1)), |backend, ui| {
        let child = ui
            .open_window(PlatformSpec::new("own").icon(picture(2)), |_ui| Quiet)
            .expect("child");
        assert_eq!(backend.window_icon(child.window()), Some(picture(2)));
        child.close();
    });
}

#[test]
fn a_new_icon_set_at_run_time_is_inherited_afterwards() {
    launch_with(None, |backend, ui| {
        ui.set_window_icon(picture(3));
        assert_eq!(backend.window_icon(ui.window()), Some(picture(3)));
        let child = ui
            .open_window(PlatformSpec::new("child"), |_ui| Quiet)
            .expect("child");
        assert_eq!(backend.window_icon(child.window()), Some(picture(3)));
        child.close();
    });
}
