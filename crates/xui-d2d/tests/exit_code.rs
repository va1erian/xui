#![cfg(windows)]

//! A d2d app that shows text and quits must leave the process with exit code 0.
//!
//! Cached Direct2D objects used to be destroyed by thread-local destructors
//! during process teardown, where DXGI raised `DXGI_ERROR_INVALID_CALL`
//! (`0x887A0001`) and the process exited with that code instead. Only the
//! main thread's teardown shows it, so this test has no harness: it runs the
//! app in a child process and checks its status. It opens a real window, so
//! run it in the sandbox (`scripts\sandbox\run.ps1`).

use std::process::Command;
use std::rc::Rc;

use xui_core::prelude::*;
use xui_d2d::D2dBackend;

const CHILD: &str = "XUI_EXIT_CODE_CHILD";

struct Hello;

impl App for Hello {
    type Msg = ();

    fn update(&mut self, _msg: (), _ui: &mut Ui<()>) {}
}

fn child() -> Result<()> {
    xui_core::app("Exit code")
        .backend(Rc::new(D2dBackend::new()))
        .size(240, 120)
        .run(|ui| {
            ui.root(column().padding(8).children((label("Hello"), edit())))?;
            Ok(Hello)
        })
}

fn main() {
    if std::env::var_os(CHILD).is_some() {
        child().expect("the app ran");
        return;
    }
    let status = Command::new(std::env::current_exe().expect("the test's own path"))
        .env(CHILD, "1")
        .env("XUI_DEMO_AUTOCLOSE_MS", "500")
        .status()
        .expect("the child started");
    assert_eq!(
        status.code(),
        Some(0),
        "the app must exit cleanly, got {status}"
    );
}
