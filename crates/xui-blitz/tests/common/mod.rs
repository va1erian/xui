//! A `BlitzView` on the offscreen backend: the tests build one, wait for it to
//! settle (with a deadline, so a hang fails), poke it and read its pixels and
//! events.

#![allow(dead_code)]

use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use xui_blitz::{BlitzView, BlitzViewBuilder, BlitzViewEvent};
use xui_canvas::snapshot::{Snapshot, Stage, render_with};
use xui_core::Dip;
use xui_core::app::{App, Ui};
use xui_core::image::Image;
use xui_core::theme::Theme;

/// How long any wait may take before the test fails.
const DEADLINE: Duration = Duration::from_secs(30);

pub enum Msg {
    Frame,
    Event(BlitzViewEvent),
    /// Calls `BlitzView::navigate`.
    Navigate(String),
}

pub struct Host {
    pub view: BlitzView<Msg>,
    pub events: Rc<RefCell<Vec<BlitzViewEvent>>>,
}

impl App for Host {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, _: &mut Ui<Msg>) {
        match msg {
            Msg::Frame => self.view.update(),
            Msg::Event(e) => self.events.borrow_mut().push(e),
            Msg::Navigate(url) => self.view.navigate(&url),
        }
    }
}

/// A view builder that maps every event.
pub fn builder() -> BlitzViewBuilder<Msg> {
    BlitzView::builder(|| Msg::Frame, |e| Some(Msg::Event(e)))
}

/// What a test sees while the view runs.
pub struct Probe<'a, 's> {
    pub stage: &'a Stage<'s, Msg>,
    pub events: Rc<RefCell<Vec<BlitzViewEvent>>>,
}

impl Probe<'_, '_> {
    /// Repaints and pumps until `done` holds; panics at the deadline.
    pub fn wait(&self, what: &str, mut done: impl FnMut(&[BlitzViewEvent]) -> bool) {
        let start = Instant::now();
        loop {
            let _ = self.stage.ui().capture();
            self.stage.emit(Msg::Frame);
            if done(&self.events.borrow()) {
                return;
            }
            assert!(
                start.elapsed() < DEADLINE,
                "timed out waiting for {what}; events: {:?}",
                self.events.borrow()
            );
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    /// Waits until a load has finished (the newest `LoadingChanged` is
    /// `false`) and then until no new frame arrives for a moment.
    pub fn wait_loaded(&self) {
        self.wait("the page to load", |events| {
            events
                .iter()
                .rev()
                .find_map(|e| match e {
                    BlitzViewEvent::LoadingChanged(busy) => Some(!busy),
                    _ => None,
                })
                .unwrap_or(false)
        });
        for _ in 0..10 {
            let _ = self.stage.ui().capture();
            self.stage.emit(Msg::Frame);
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    pub fn take_events(&self) -> Vec<BlitzViewEvent> {
        std::mem::take(&mut self.events.borrow_mut())
    }
}

/// Runs `test` against a 400×300 view made by `make`, then returns the final
/// frame.
pub fn run(
    theme: Theme,
    make: impl FnOnce() -> BlitzViewBuilder<Msg> + 'static,
    test: impl FnOnce(&Probe<'_, '_>) + 'static,
) -> Image {
    let events = Rc::new(RefCell::new(Vec::new()));
    let seen = Rc::clone(&events);
    render_with(
        Snapshot::new(Dip(400.0), Dip(300.0)).theme(theme),
        move |ui| {
            let view = make().build(ui, ui.client_rect())?;
            Ok(Host { view, events })
        },
        move |stage| {
            test(&Probe {
                stage,
                events: seen,
            })
        },
    )
    .expect("render")
}

/// The pixel at `(x, y)` without its alpha.
pub fn rgb(image: &Image, x: u32, y: u32) -> [u8; 3] {
    let [r, g, b, _] = image.pixel(x, y).expect("in the image");
    [r, g, b]
}
