//! A headless harness for driving the editor: a host app that logs the
//! editor's messages, a stage-based runner with a watchdog, and input helpers.
#![allow(dead_code)]

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use crate::common::{DocBuilder, Obj, Run, gradient_image};
use xui_canvas::snapshot::{Snapshot, Stage, render_with};
use xui_core::app::{App, Ui};
use xui_core::backend::Event;
use xui_core::image::Image;
use xui_core::message::{Key, Modifiers, MouseButton};
use xui_core::{Color, Dip, Theme};
use xui_rich_text::edit::MemoryClipboard;
use xui_rich_text::model::{CharStyleId, DocRange, InlineImage, ParaStyleId, Wrap};
use xui_rich_text::{Document, RichTextEditor};

#[derive(Debug, Clone, PartialEq)]
pub enum Msg {
    Changed(String),
    Bold(bool),
    Link(String),
}

struct Host {
    _editor: Rc<RichTextEditor<Msg>>,
    pub log: Rc<RefCell<Vec<Msg>>>,
}

impl App for Host {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, _ui: &mut Ui<Msg>) {
        self.log.borrow_mut().push(msg);
    }
}

/// The editor and what the app heard from it.
pub struct Rig {
    pub editor: Rc<RichTextEditor<Msg>>,
    pub log: Rc<RefCell<Vec<Msg>>>,
}

impl Rig {
    pub fn text(&self) -> String {
        self.editor.with_document(|d| d.to_plain_text())
    }

    pub fn range(&self) -> DocRange {
        let sel = self.editor.selection();
        self.editor
            .with_document(|d| d.selection_range(&sel))
            .expect("a selection")
    }
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
            eprintln!("editor_input: test timed out");
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

/// Builds an editor on `doc` in a 400 x 240 window, focuses it and runs
/// `body` against it. Returns the rendered window.
pub fn run_themed(
    theme: Theme,
    doc: Document,
    body: impl FnOnce(&Stage<'_, Msg>, &Rig) + 'static,
) -> Image {
    let _watchdog = Watchdog::start();
    let slot: Rc<RefCell<Option<Rig>>> = Rc::new(RefCell::new(None));
    let build_slot = Rc::clone(&slot);
    render_with(
        Snapshot::new(Dip(400.0), Dip(240.0)).theme(theme),
        move |ui| {
            let log = Rc::new(RefCell::new(Vec::new()));
            let editor = RichTextEditor::new(ui, xui_core::Rect::new(0, 0, 400, 240))?
                .with_clipboard(Box::new(MemoryClipboard::default()))
                .on_change(|doc| Some(Msg::Changed(doc.to_plain_text())))
                .on_selection(|summary| {
                    Some(Msg::Bold(
                        summary.bold == xui_rich_text::model::Tri::Uniform(true),
                    ))
                })
                .on_link(|url| Some(Msg::Link(url.to_owned())))
                .document(doc);
            let editor = Rc::new(editor);
            *build_slot.borrow_mut() = Some(Rig {
                editor: Rc::clone(&editor),
                log: Rc::clone(&log),
            });
            Ok(Host {
                _editor: editor,
                log,
            })
        },
        move |stage| {
            let rig = slot.borrow_mut().take().expect("built");
            rig.editor.focus();
            stage.inject(Event::SetFocus);
            body(stage, &rig);
        },
    )
    .expect("render")
}

pub fn run(doc: Document, body: impl FnOnce(&Stage<'_, Msg>, &Rig) + 'static) -> Image {
    run_themed(Theme::light(), doc, body)
}

pub fn key_with(stage: &Stage<'_, Msg>, key: Key, modifiers: Modifiers) {
    stage.inject(Event::KeyDown {
        key,
        modifiers,
        repeat: 1,
        system: false,
    });
}

pub fn key(stage: &Stage<'_, Msg>, key: Key) {
    key_with(stage, key, Modifiers::NONE);
}

pub fn ctrl(stage: &Stage<'_, Msg>, k: Key) {
    key_with(
        stage,
        k,
        Modifiers {
            ctrl: true,
            ..Modifiers::NONE
        },
    );
}

pub fn shift(stage: &Stage<'_, Msg>, k: Key) {
    key_with(
        stage,
        k,
        Modifiers {
            shift: true,
            ..Modifiers::NONE
        },
    );
}

pub fn typing(stage: &Stage<'_, Msg>, text: &str) {
    text.chars().for_each(|c| {
        stage.inject(Event::Char(c));
    });
}

pub fn mouse(stage: &Stage<'_, Msg>, kind: &str, x: i32, y: i32) {
    let modifiers = Modifiers::NONE;
    let button = MouseButton::Left;
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
        "double" => Event::MouseDoubleClick {
            x,
            y,
            button,
            modifiers,
        },
        _ => Event::MouseMove { x, y, modifiers },
    });
}

pub fn plain(text: &str) -> Document {
    let mut b = DocBuilder::new();
    b.paragraph(
        ParaStyleId::DEFAULT,
        &[Run::Text(text, CharStyleId::DEFAULT)],
    );
    b.finish()
}

pub fn picture(b: &mut DocBuilder, w: f32, h: f32) -> Obj {
    b.object(InlineImage {
        image: gradient_image(8, 8, Color::rgb(30, 90, 200), Color::rgb(10, 30, 90)),
        size: (Dip(w), Dip(h)),
        wrap: Wrap::Inline,
        alt: String::new(),
    })
}

pub fn image_size(rig: &Rig) -> (f32, f32) {
    rig.editor.with_document(|d| {
        let id = d.paragraphs()[0].anchors()[0];
        let size = d.objects().get(id).expect("object").size;
        (size.0.0, size.1.0)
    })
}
pub fn image_doc() -> Document {
    let mut b = DocBuilder::new();
    let img = picture(&mut b, 60.0, 40.0);
    b.paragraph(ParaStyleId::DEFAULT, &[Run::Object(img)]);
    b.finish()
}

/// How many pixels of the window are exactly `color`.
pub fn pixels_of(image: &Image, color: Color) -> usize {
    image
        .pixels()
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|p| p[..3] == [color.r, color.g, color.b])
        .count()
}

/// Whether some pixel column holds `min` consecutive pixels of `color`.
pub fn has_column(image: &Image, color: Color, min: u32) -> bool {
    (0..image.width()).any(|x| {
        let mut run = 0;
        (0..image.height()).any(|y| {
            let hit = image
                .pixel(x, y)
                .is_some_and(|p| p[..3] == [color.r, color.g, color.b]);
            run = if hit { run + 1 } else { 0 };
            run >= min
        })
    })
}

pub fn save(name: &str, image: &Image) {
    let dir = std::path::Path::new("../../target/snapshots");
    std::fs::create_dir_all(dir).expect("snapshot directory");
    image.save_png(dir.join(name)).expect("save");
}
