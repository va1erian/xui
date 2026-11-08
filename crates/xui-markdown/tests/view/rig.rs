//! The harness: a view in a headless window, a watchdog and input helpers.
#![allow(dead_code)]

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use xui_canvas::snapshot::{Snapshot, Stage, render_with};
use xui_core::app::{App, Ui};
use xui_core::backend::Event;
use xui_core::image::Image;
use xui_core::message::{Key, Modifiers, MouseButton};
use xui_core::{Dip, Rect, Theme};
use xui_markdown::MarkdownView;

pub const SAMPLE: &str = include_str!("../sample.md");
pub const WIDTH: i32 = 640;
pub const HEIGHT: i32 = 300;

struct Host {
    _view: Rc<MarkdownView<String>>,
    log: Rc<RefCell<Vec<String>>>,
}

impl App for Host {
    type Msg = String;

    fn update(&mut self, msg: String, _ui: &mut Ui<String>) {
        self.log.borrow_mut().push(msg);
    }
}

/// The view and the link destinations the app heard.
pub struct Rig {
    pub view: Rc<MarkdownView<String>>,
    pub log: Rc<RefCell<Vec<String>>>,
}

/// Fails the test run instead of hanging if a test is stuck.
struct Watchdog(Arc<AtomicBool>);

impl Watchdog {
    fn start() -> Watchdog {
        let done = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&done);
        std::thread::spawn(move || {
            for _ in 0..300 {
                std::thread::sleep(Duration::from_millis(100));
                if flag.load(Ordering::Relaxed) {
                    return;
                }
            }
            eprintln!("markdown view: test timed out");
            std::process::exit(101);
        });
        Watchdog(done)
    }
}

impl Drop for Watchdog {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Relaxed);
    }
}

/// Shows `markdown` in a 640 x 300 window, scrolled to the very top, and runs
/// `body` against it.
pub fn run(markdown: &str, body: impl FnOnce(&Stage<'_, String>, &Rig) + 'static) -> Image {
    run_raw(markdown, |stage, rig| {
        rig.view.set_scroll(0.0);
        paint(stage);
        body(stage, rig);
    })
}

/// Like [`run`], with the view as the first paint leaves it.
pub fn run_raw(markdown: &str, body: impl FnOnce(&Stage<'_, String>, &Rig) + 'static) -> Image {
    let _watchdog = Watchdog::start();
    let slot: Rc<RefCell<Option<Rig>>> = Rc::new(RefCell::new(None));
    let build_slot = Rc::clone(&slot);
    let markdown = markdown.to_owned();
    render_with(
        Snapshot::new(Dip(WIDTH as f32), Dip(HEIGHT as f32)).theme(Theme::light()),
        move |ui| {
            let log = Rc::new(RefCell::new(Vec::new()));
            let view = MarkdownView::new(ui, Rect::new(0, 0, WIDTH, HEIGHT))?
                .on_link(|href| Some(href.to_owned()))
                .on_context(|at| Some(format!("context:{},{}", at.x, at.y)))
                .markdown(&markdown);
            let view = Rc::new(view);
            *build_slot.borrow_mut() = Some(Rig {
                view: Rc::clone(&view),
                log: Rc::clone(&log),
            });
            Ok(Host { _view: view, log })
        },
        move |stage| {
            let rig = slot.borrow_mut().take().expect("built");
            paint(stage);
            body(stage, &rig);
        },
    )
    .expect("render")
}

/// Paints a frame, as a real window would after every event: the view only
/// lays out and settles its scroll position while painting.
pub fn paint(stage: &Stage<'_, String>) {
    stage.ui().capture().expect("capture");
}

/// Presses End and returns where it landed: on the real end at once, even
/// when the blocks there were only estimated before.
pub fn to_end(stage: &Stage<'_, String>, rig: &Rig) -> f32 {
    key(stage, Key::END);
    rig.view.scroll()
}

pub fn focus(stage: &Stage<'_, String>, rig: &Rig) {
    rig.view.focus();
    stage.inject(Event::SetFocus);
}

pub fn key(stage: &Stage<'_, String>, key: Key) {
    stage.inject(Event::KeyDown {
        key,
        modifiers: Modifiers::NONE,
        repeat: 1,
        system: false,
    });
    paint(stage);
}

pub fn wheel(stage: &Stage<'_, String>, notches: i16) {
    stage.inject(Event::MouseWheel {
        delta: notches * 120,
        horizontal: false,
        x: 100,
        y: 100,
        modifiers: Modifiers::NONE,
    });
    paint(stage);
}

pub fn mouse(stage: &Stage<'_, String>, kind: &str, x: i32, y: i32) {
    let (modifiers, button) = (Modifiers::NONE, MouseButton::Left);
    stage.inject(match kind {
        "down" => Event::MouseDown {
            x,
            y,
            button,
            modifiers,
        },
        "up" => Event::MouseUp {
            x,
            y,
            button,
            modifiers,
        },
        _ => Event::MouseMove { x, y, modifiers },
    });
    paint(stage);
}

/// Clicks a grid over the text area until `found` holds, and returns where.
pub fn find(stage: &Stage<'_, String>, found: impl Fn() -> bool) -> Option<(i32, i32)> {
    for y in (16..200).step_by(3) {
        for x in (16..WIDTH - 40).step_by(4) {
            stage.click(x, y);
            if found() {
                return Some((x, y));
            }
        }
    }
    None
}

pub fn clicked_next(rig: &Rig) -> bool {
    rig.log.borrow().iter().any(|href| href == "next.md")
}

/// Presses `key` with Ctrl held.
pub fn ctrl(stage: &Stage<'_, String>, key: Key) {
    stage.inject(Event::KeyDown {
        key,
        modifiers: Modifiers {
            ctrl: true,
            ..Modifiers::NONE
        },
        repeat: 1,
        system: false,
    });
    paint(stage);
}

/// A mouse event of `kind` (`down`, `up`, anything else is a move) with
/// `button` and `modifiers`.
pub fn mouse_with(
    stage: &Stage<'_, String>,
    kind: &str,
    x: i32,
    y: i32,
    button: MouseButton,
    modifiers: Modifiers,
) {
    stage.inject(match kind {
        "down" => Event::MouseDown {
            x,
            y,
            button,
            modifiers,
        },
        "up" => Event::MouseUp {
            x,
            y,
            button,
            modifiers,
        },
        _ => Event::MouseMove { x, y, modifiers },
    });
    paint(stage);
}

/// Presses the left button with Shift held, then releases it.
pub fn shift_click(stage: &Stage<'_, String>, x: i32, y: i32) {
    let shift = Modifiers {
        shift: true,
        ..Modifiers::NONE
    };
    mouse_with(stage, "down", x, y, MouseButton::Left, shift);
    mouse_with(stage, "up", x, y, MouseButton::Left, shift);
}

/// A press and release of the right button.
pub fn right_click(stage: &Stage<'_, String>, x: i32, y: i32) {
    mouse_with(stage, "down", x, y, MouseButton::Right, Modifiers::NONE);
    mouse_with(stage, "up", x, y, MouseButton::Right, Modifiers::NONE);
}

/// What a window sends for a double click: the first click, then the
/// double-click event in place of the second press, then the release.
pub fn double_click(stage: &Stage<'_, String>, x: i32, y: i32) {
    stage.click(x, y);
    stage.inject(Event::MouseDoubleClick {
        x,
        y,
        button: MouseButton::Left,
        modifiers: Modifiers::NONE,
    });
    paint(stage);
    mouse(stage, "up", x, y);
}

/// Drags the left button from `from` to `to` with a move between.
pub fn drag(stage: &Stage<'_, String>, from: (i32, i32), to: (i32, i32)) {
    mouse(stage, "down", from.0, from.1);
    mouse(stage, "move", (from.0 + to.0) / 2, (from.1 + to.1) / 2);
    mouse(stage, "move", to.0, to.1);
    mouse(stage, "up", to.0, to.1);
}

/// The text on the window's clipboard.
pub fn clipboard(stage: &Stage<'_, String>) -> Option<String> {
    stage.ui().clipboard_text()
}
