//! Frame cost of the canvas backend's damage-driven repaint, at 1080p.
//!
//! A 1920x1080 offscreen window holds one card (a `Panel` with a title, an
//! `Edit`, a `Button` and 30 small widgets in a grid: about 400x300, 34 nodes).
//! The example times, per frame, over `--frames` frames (default 300) after a
//! warm-up:
//!
//! - an idle frame (nothing changed),
//! - the card moved by one pixel per frame,
//! - a forced repaint of the whole window,
//! - the whole-window repaint, image clone and conversion that the windowed
//!   backend's present path did for every frame before it repainted damage.
//!
//! Each timed frame includes what the windowed present does with the result:
//! converting the repainted rows to the `0x00RRGGBB` buffer `softbuffer` takes.
//!
//! Run it in release: `cargo run --release -p xui-canvas --example
//! damage_bench [-- --frames 300 --window 1920x1080]`. The numbers depend on
//! the machine; a debug build is an order of magnitude slower.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Instant;

use xui_canvas::{OffscreenBackend, PresentedFrame};
use xui_core::app::{App, Ui, run_app};
use xui_core::arrange::{
    Entry, Handle, IntoEntry, Layout, LayoutExt, Track, absolute, button, checkbox, column, edit,
    grid, label, panel,
};
use xui_core::backend::{Backend, PlatformSpec, WidgetId};
use xui_core::widget::Panel;
use xui_core::{Dip, Rect};

/// What the build hands the timed loop: the window and the card's node.
type Built = (Ui<()>, WidgetId);

struct Nothing;

impl App for Nothing {
    type Msg = ();

    fn update(&mut self, _msg: (), _ui: &mut Ui<()>) {}
}

/// The card: a title, an `Edit`, 30 small widgets in a 5-column grid and a
/// `Button`.
fn card() -> Layout<()> {
    let cells: Vec<Entry<()>> = (0..30)
        .map(|i| match i % 3 {
            0 => checkbox(format!("Opt {i}")).into_entry(),
            1 => label(format!("Item {i}")).into_entry(),
            _ => button(format!("B{i}")).into_entry(),
        })
        .collect();
    column().padding(10).gap(6).children((
        label("Card").title(),
        edit().text("hello world"),
        grid(vec![Track::Auto; 5]).gap(4).children(cells),
        button("Apply").primary(),
    ))
}

struct Stats {
    median: f64,
    p95: f64,
    max: f64,
}

/// Times `frames` calls of `each` after `warmup` untimed ones, in ms.
fn measure(frames: usize, warmup: usize, mut each: impl FnMut(usize)) -> Stats {
    for i in 0..warmup {
        each(i);
    }
    let mut ms = Vec::with_capacity(frames);
    for i in 0..frames {
        let start = Instant::now();
        each(warmup + i);
        ms.push(start.elapsed().as_secs_f64() * 1000.0);
    }
    ms.sort_by(|a, b| a.total_cmp(b));
    let last = ms.len() - 1;
    Stats {
        median: ms[ms.len() / 2],
        p95: ms[((ms.len() as f64 * 0.95) as usize).min(last)],
        max: ms[last],
    }
}

fn report(name: &str, stats: &Stats) {
    println!(
        "{name:<58} median {:7.3} ms   p95 {:7.3} ms   max {:7.3} ms",
        stats.median, stats.p95, stats.max
    );
}

/// What the windowed present does with a frame: convert the repainted rows
/// (all of them when `whole`) to `0x00RRGGBB`.
fn convert(frame: &PresentedFrame<'_>, buffer: &mut [u32], whole: bool) {
    let width = frame.width as usize;
    let window = [Rect::new(0, 0, frame.width as i32, frame.height as i32)];
    let rects: &[Rect] = if whole { &window } else { frame.damage };
    for rect in rects {
        for row in rect.top as usize..rect.bottom as usize {
            let from = &frame.pixels
                [(row * width + rect.left as usize) * 4..(row * width + rect.right as usize) * 4];
            let to =
                &mut buffer[row * width + rect.left as usize..row * width + rect.right as usize];
            for (destination, pixel) in to.iter_mut().zip(from.as_chunks::<4>().0) {
                *destination =
                    (u32::from(pixel[0]) << 16) | (u32::from(pixel[1]) << 8) | u32::from(pixel[2]);
            }
        }
    }
}

fn argument(name: &str, default: &str) -> String {
    let args: Vec<String> = std::env::args().collect();
    args.iter()
        .position(|arg| arg == name)
        .and_then(|at| args.get(at + 1).cloned())
        .unwrap_or_else(|| default.to_string())
}

fn main() {
    let frames: usize = argument("--frames", "300").parse().expect("a frame count");
    let size = argument("--window", "1920x1080");
    let (width, height) = size.split_once('x').expect("WIDTHxHEIGHT");
    let (width, height): (u32, u32) = (
        width.parse().expect("a width"),
        height.parse().expect("a height"),
    );

    let backend = Rc::new(OffscreenBackend::new());
    let slot: Rc<RefCell<Option<Built>>> = Rc::new(RefCell::new(None));
    let weak = Rc::downgrade(&backend);
    let hook_slot = Rc::clone(&slot);
    backend.set_run_hook(move || {
        let backend = weak.upgrade().expect("the backend");
        let (ui, card) = hook_slot.borrow_mut().take().expect("the app was built");
        bench(&backend, &ui, card, (width, height), frames);
    });

    let make_slot = Rc::clone(&slot);
    let run: Rc<dyn Backend> = backend.clone();
    run_app(
        run,
        PlatformSpec::new("damage bench").size(Dip(width as f32), Dip(height as f32)),
        move |ui| {
            let card_handle: Handle<Panel<()>> = Handle::new();
            ui.root(
                absolute().design_size(width as f32, height as f32).child(
                    panel(card())
                        .bind(&card_handle)
                        .at(100.0, 100.0, 400.0, 300.0),
                ),
            )
            .expect("the card mounts");
            *make_slot.borrow_mut() = Some((ui.clone(), card_handle.get().id()));
            Nothing
        },
    )
    .expect("the app ran");
}

fn bench(
    backend: &OffscreenBackend,
    ui: &Ui<()>,
    card: WidgetId,
    (width, height): (u32, u32),
    frames: usize,
) {
    let window = ui.window();
    ui.relayout();
    let rect = ui.bounds(card);
    println!(
        "{} build, window {width}x{height}, card {}x{} at ({}, {}), {frames} frames",
        if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        },
        rect.width(),
        rect.height(),
        rect.left,
        rect.top,
    );
    let mut buffer = vec![0u32; (width * height) as usize];

    let start = Instant::now();
    backend.present(window, |frame| convert(&frame, &mut buffer, true));
    println!(
        "first frame (cold, everything painted): {:.3} ms",
        start.elapsed().as_secs_f64() * 1000.0
    );

    report(
        "idle frame",
        &measure(frames, 20, |_| {
            backend.present(window, |frame| convert(&frame, &mut buffer, false));
        }),
    );

    let mut damage = Rect::default();
    report(
        "card moved 1 px per frame",
        &measure(frames, 20, |i| {
            let dx = i as i32 % 200;
            ui.apply_moves(&[(
                card,
                Rect::new(rect.left + dx, rect.top, rect.right + dx, rect.bottom),
            )]);
            backend.present(window, |frame| {
                damage = frame.damage.first().copied().unwrap_or_default();
                convert(&frame, &mut buffer, false);
            });
        }),
    );
    println!(
        "    damage of a moved frame: {}x{} px ({:.1}% of the window)",
        damage.width(),
        damage.height(),
        100.0 * f64::from(damage.width()) * f64::from(damage.height())
            / (f64::from(width) * f64::from(height))
    );

    report(
        "forced repaint of the whole window",
        &measure(frames, 20, |_| {
            backend.invalidate_window(window);
            backend.present(window, |frame| convert(&frame, &mut buffer, true));
        }),
    );

    report(
        "whole repaint + image clone + conversion (the old present)",
        &measure(frames, 20, |_| {
            backend.invalidate_window(window);
            let image = backend.render(window).expect("a frame");
            for (destination, pixel) in buffer.iter_mut().zip(image.pixels.as_chunks::<4>().0) {
                *destination =
                    (u32::from(pixel[0]) << 16) | (u32::from(pixel[1]) << 8) | u32::from(pixel[2]);
            }
        }),
    );
}
