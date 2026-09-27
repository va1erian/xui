#![forbid(unsafe_code)]

//! Unit tests for the window backend that do not need a live window.

use super::{Decorations, PlatformSpec, WindowState};
use xui_core::units::dip;

#[test]
fn window_state_records_the_requested_chrome() {
    let spec = PlatformSpec::new("t")
        .decorations(Decorations::None)
        .caption_inset(dip(36.0));
    let state = WindowState::new(&spec);
    assert_eq!(state.decorations, Decorations::None);
    assert_eq!(state.caption_inset, dip(36.0));
}

#[test]
fn a_default_window_keeps_its_decorations() {
    let state = WindowState::new(&PlatformSpec::new("t"));
    assert_eq!(state.decorations, Decorations::System);
    assert_eq!(state.caption_inset, dip(0.0));
}

#[test]
fn window_state_records_whether_the_window_is_resizable() {
    assert!(WindowState::new(&PlatformSpec::new("t")).resizable);
    let mut spec = PlatformSpec::new("t");
    spec.resizable = false;
    assert!(!WindowState::new(&spec).resizable);
}
