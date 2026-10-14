use super::*;

#[test]
fn reduced_motion_is_off_by_default() {
    let (backend, _window, _core, ui) = setup();
    assert!(!backend.prefers_reduced_motion());
    assert!(!ui.prefers_reduced_motion());
}

#[test]
fn a_backend_that_prefers_reduced_motion_is_visible_through_the_ui() {
    let (backend, _window, _core, ui) = setup();
    backend.set_prefers_reduced_motion(true);
    assert!(ui.prefers_reduced_motion());
}

#[test]
fn the_preference_is_read_live_not_cached() {
    let (backend, _window, _core, ui) = setup();
    backend.set_prefers_reduced_motion(true);
    assert!(ui.prefers_reduced_motion());
    backend.set_prefers_reduced_motion(false);
    assert!(
        !ui.prefers_reduced_motion(),
        "a change made while the app runs applies to the next animation"
    );
}

#[test]
fn every_handle_on_the_window_sees_the_same_preference() {
    let (backend, _window, _core, ui) = setup();
    let scoped = ui.with_parent(WidgetId::from_raw(1));
    backend.set_prefers_reduced_motion(true);
    assert!(scoped.prefers_reduced_motion());
    assert!(ui.clone().prefers_reduced_motion());
}
