//! Layout-tree tests against real widgets: the tree positions the children and
//! `Ui::relayout` re-runs it when something changes.
//!
//! Pure tree-to-rects arithmetic is covered by the unit tests in
//! `src/app/layout.rs`; this exercises the window-owned path.

#![cfg(windows)]

mod common;

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use common::run_app_with_watchdog;
use xui_win32::prelude::*;
use xui_win32::{Anchor, Layout, Size, column, row};

enum Msg {
    Start,
    Hide,
    Check,
}

struct LayoutApp {
    left: Option<Label>,
    right: Option<Label>,
    initial: Rc<RefCell<Vec<(Rect, Rect)>>>,
    after_hide: Rc<RefCell<Vec<(Rect, Rect)>>>,
}

impl App for LayoutApp {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, ui: &mut Ui<Msg>) {
        let (Some(left), Some(right)) = (&self.left, &self.right) else {
            ui.quit();
            return;
        };
        match msg {
            Msg::Start => {
                self.initial
                    .borrow_mut()
                    .push((left.bounds(), right.bounds()));
                ui.emit(Msg::Hide);
            }
            Msg::Hide => {
                left.set_visible(false);
                ui.relayout();
                self.after_hide
                    .borrow_mut()
                    .push((left.bounds(), right.bounds()));
                ui.emit(Msg::Check);
            }
            Msg::Check => ui.quit(),
        }
    }
}

/// A row of a fixed-width and a filling widget: the tree must place them side
/// by side, and hiding the fixed one must give its space back.
#[test]
fn layout_positions_widgets_and_relayouts() {
    let client = Rc::new(Cell::new(Rect::default()));
    let initial = Rc::new(RefCell::new(Vec::new()));
    let after_hide = Rc::new(RefCell::new(Vec::new()));
    let created = Rc::new(Cell::new(false));

    let client_for_make = Rc::clone(&client);
    let initial_for_make = Rc::clone(&initial);
    let after_hide_for_make = Rc::clone(&after_hide);
    let created_for_make = Rc::clone(&created);

    let Some(run) = run_app_with_watchdog("win32ui.layout", move |ui| {
        let dpi = ui.dpi();
        client_for_make.set(ui.client_rect());
        let left = Label::new(ui, Rect::default(), "left").ok();
        let right = Label::new(ui, Rect::default(), "right").ok();

        if let (Some(left), Some(right)) = (&left, &right) {
            let fixed = dip(80.0).to_px(dpi).value();
            ui.set_layout(column![row![left.width(dip(80.0)), right.fill(1)].fill(1)]);
            assert_eq!(
                left.bounds().width(),
                fixed,
                "the fixed width was not honoured"
            );
            initial_for_make
                .borrow_mut()
                .push((left.bounds(), right.bounds()));
            created_for_make.set(true);
            ui.emit(Msg::Start);
        } else {
            ui.quit();
        }

        LayoutApp {
            left,
            right,
            initial: initial_for_make,
            after_hide: after_hide_for_make,
        }
    }) else {
        return;
    };

    assert!(!run.timed_out, "the watchdog fired before the app quit");
    if !created.get() {
        return;
    }

    let client = client.get();
    let initial = initial.borrow();
    let (left, right) = initial.last().expect("initial bounds");
    assert_eq!(left.left, 0);
    assert_eq!(left.top, 0);
    assert_eq!(left.right, right.left, "the row's items must be adjacent");
    assert_eq!(right.right, client.right, "the row must fill the width");
    drop(initial);

    let after_hide = after_hide.borrow();
    let (_, right) = after_hide.last().expect("bounds after hiding");
    assert_eq!(
        right.left, 0,
        "hiding the fixed widget did not reclaim its space"
    );
    assert_eq!(
        right.right, client.right,
        "the filling widget should now span the row"
    );
}

struct FreeApp {
    full: Option<Label>,
    wide: Option<Label>,
    before: Rc<Cell<Option<(Rect, Rect)>>>,
    after: Rc<Cell<Option<(Rect, Rect)>>>,
}

impl App for FreeApp {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, ui: &mut Ui<Msg>) {
        let (Some(full), Some(wide)) = (&self.full, &self.wide) else {
            ui.quit();
            return;
        };
        match msg {
            Msg::Start => {
                self.before.set(Some((full.bounds(), wide.bounds())));
                // Grow the window by 100x80 design units and let the resize
                // relayout run; `set_placement` sends `WM_SIZE` synchronously.
                let mut placement = ui.placement();
                let grow = dip(100.0).to_px(ui.dpi()).value();
                let grow_y = dip(80.0).to_px(ui.dpi()).value();
                placement.normal.right += grow;
                placement.normal.bottom += grow_y;
                let _ = ui.set_placement(&placement);
                self.after.set(Some((full.bounds(), wide.bounds())));
                ui.emit(Msg::Check);
            }
            Msg::Hide => {}
            Msg::Check => ui.quit(),
        }
    }
}

/// A free (absolute) layout on a real window: a `Fill` and a
/// `StretchHorizontal` label must follow a real resize, staying inside the new
/// client area.
#[test]
fn free_layout_follows_a_real_window_resize() {
    let before = Rc::new(Cell::new(None));
    let after = Rc::new(Cell::new(None));
    let checked = Rc::new(Cell::new(false));

    let before_for_make = Rc::clone(&before);
    let after_for_make = Rc::clone(&after);
    let checked_for_make = Rc::clone(&checked);

    let Some(run) = run_app_with_watchdog("win32ui.free_layout", move |ui| {
        let dpi = ui.dpi();
        let px = |value: f32| dip(value).to_px(dpi).value();
        let client = ui.client_rect();
        // The design origin is the window's client size in design pixels, so
        // installing the layout does not move the controls.
        let origin = Size::new(
            Px(client.width()).to_dip(dpi).to_px(96).value(),
            Px(client.height()).to_dip(dpi).to_px(96).value(),
        );

        let full = Label::new(ui, Rect::default(), "full").ok();
        let wide = Label::new(ui, Rect::default(), "wide").ok();
        if let (Some(full), Some(wide)) = (&full, &wide) {
            full.set_bounds(Rect::new(px(10.0), px(10.0), px(90.0), px(50.0)));
            wide.set_bounds(Rect::new(px(10.0), px(100.0), px(90.0), px(140.0)));
            ui.set_layout(
                Layout::free(origin)
                    .item(full.anchor(Anchor::Fill))
                    .item(wide.anchor(Anchor::StretchHorizontal)),
            );
            checked_for_make.set(true);
            ui.emit(Msg::Start);
        } else {
            ui.quit();
        }

        FreeApp {
            full,
            wide,
            before: before_for_make,
            after: after_for_make,
        }
    }) else {
        return;
    };

    assert!(!run.timed_out, "the watchdog fired before the app quit");
    if !checked.get() {
        return;
    }

    let (full_before, wide_before) = before.get().expect("bounds before the resize");
    let (full_after, wide_after) = after.get().expect("bounds after the resize");
    assert_eq!(
        full_after.left, full_before.left,
        "a Fill control is left-pinned"
    );
    assert_eq!(
        full_after.top, full_before.top,
        "a Fill control is top-pinned"
    );
    assert!(
        full_after.width() > full_before.width(),
        "the Fill control did not grow with the window: {full_before:?} -> {full_after:?}"
    );
    assert!(
        full_after.height() > full_before.height(),
        "the Fill control did not grow in height: {full_before:?} -> {full_after:?}"
    );
    assert_eq!(
        wide_after.left, wide_before.left,
        "a StretchHorizontal control is left-pinned"
    );
    assert!(
        wide_after.width() > wide_before.width(),
        "the StretchHorizontal control did not track the width: {wide_before:?} -> {wide_after:?}"
    );
    assert_eq!(
        wide_after.height(),
        wide_before.height(),
        "a StretchHorizontal control must not grow in height"
    );
}
