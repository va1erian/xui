//! Popup surfaces and lifecycle.

use super::*;

#[test]
fn popups_are_created_as_transient_popup_surfaces() {
    let (backend, _core, ui) = setup();
    let menu = sample_bar(&ui);
    let bar = menu.id().unwrap();
    let popup = menu.popup_id(0).unwrap();

    assert!(
        backend.is_popup(popup),
        "a dropdown is a transient surface the backend can float above the window"
    );
    assert!(!backend.is_popup(bar), "the bar itself is an ordinary node");
}

#[test]
fn moving_over_the_open_title_does_not_reopen_the_popup() {
    let (backend, core, ui) = setup();
    let menu = sample_bar(&ui);
    let bar = menu.id().unwrap();
    let popup = menu.popup_id(0).unwrap();
    let runtime = Runtime::primary(core, TestApp(Rc::new(RefCell::new(Vec::new()))));

    runtime.deliver(bar, &down(5, 14));
    assert!(menu.is_open());

    // The pointer moves over the bar arrive continuously. Moving inside the
    // title whose menu is already open must not hide and re-show the popup,
    // or the drop-down flickers on every move.
    let before = backend.move_calls();
    for x in 6..=10 {
        runtime.deliver(
            bar,
            &Event::MouseMove {
                x,
                y: 14,
                modifiers: Modifiers::NONE,
            },
        );
    }
    assert_eq!(
        backend.move_calls(),
        before,
        "moving within the open title must not reposition the popup"
    );
    assert!(visible(&backend, popup), "the popup stayed shown");
}

#[test]
fn dropping_a_menu_unregisters_its_mappers() {
    let (_backend, core, ui) = setup();
    let menu = sample_bar(&ui);
    assert!(!core.router().is_empty(), "the mappers are registered");

    drop(menu);
    assert!(
        core.router().is_empty(),
        "the bar and popup mappers were unregistered"
    );
}
