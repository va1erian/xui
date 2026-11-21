#![forbid(unsafe_code)]

//! The tab strip of [`Tabs`](super::Tabs): painting, input and the accent
//! underline that travels to the newly selected tab.

use std::rc::Rc;
use std::time::Duration;

use super::{ACCENT, PADDING, Shared, TEXT, lay_out, slide};
use crate::app::Ui;
use crate::backend::{Canvas, Event, TextStyle, WidgetId};
use crate::geometry::{Point, Rect};
use crate::message::MouseButton;
use crate::motion::{Animated, Motion};
use crate::theme::Theme;
use crate::theme::look;

/// The selected tab's accent underline: its left and right edges in the
/// strip's own coordinates, each easing to where the selected tab is.
pub(super) struct Underline {
    left: Rc<Animated>,
    right: Rc<Animated>,
}

impl Underline {
    pub(super) fn new() -> Underline {
        Underline {
            left: Rc::new(Animated::new(0.0)),
            right: Rc::new(Animated::new(0.0)),
        }
    }

    /// Puts the underline under `tab` at once, unless it is already there or
    /// on its way there: a layout pass that follows a selection must not cut
    /// the travel short.
    pub(super) fn settle(&self, tab: Rect) {
        let (left, right) = (tab.left as f32, tab.right as f32);
        if self.left.target() != left || self.right.target() != right {
            self.left.jump(left);
            self.right.jump(right);
        }
    }

    /// Slides the underline to `tab` from where it is now, even mid-flight.
    fn travel_to<M: 'static>(&self, ui: &Ui<M>, strip: WidgetId, tab: Rect) {
        ui.animate(strip, &self.left, tab.left as f32, Motion::TRAVEL);
        ui.animate(strip, &self.right, tab.right as f32, Motion::TRAVEL);
    }

    /// The edges at `now`, in whole pixels.
    fn edges(&self, now: Duration) -> (i32, i32) {
        (
            self.left.value(now).round() as i32,
            self.right.value(now).round() as i32,
        )
    }
}

/// Selects `index` and slides the underline to it; `false` when `index` is out
/// of range or already selected. The change event is the caller's.
pub(super) fn select_tab<M: 'static>(ui: &Ui<M>, s: &Rc<Shared<M>>, index: usize) -> bool {
    let previous = s.selected.get();
    if index >= s.pages.borrow().len() || previous == index {
        return false;
    }
    // A scroll still in flight lands before the next one starts.
    s.slide.end(ui, s.id);
    s.selected.set(index);
    if let Some(tab) = lay_out(ui, s) {
        s.underline.travel_to(ui, s.strip_id, tab);
    }
    slide::start(ui, s, previous);
    true
}

/// Selects `index` for a click and raises the change event.
fn choose<M: 'static>(ui: &Ui<M>, s: &Rc<Shared<M>>, index: usize) {
    if !select_tab(ui, s, index) {
        return;
    }
    s.hover.set(None);
    let mapped = s.on_change.borrow().as_ref().and_then(|f| f(index));
    if let Some(msg) = mapped {
        ui.emit(msg);
    }
}

/// The tab under `x` (the strip's own coordinate).
fn tab_at<M>(s: &Shared<M>, x: i32) -> Option<usize> {
    s.tabs
        .borrow()
        .iter()
        .position(|rect| x >= rect.left && x < rect.right)
}

/// Paints the strip: each tab's background, title and the selected underline.
pub(super) fn paint_strip<M>(s: &Shared<M>, canvas: &mut dyn Canvas, theme: Theme, now: Duration) {
    let bounds = canvas.bounds();
    look::band(canvas, &theme);
    // Tab rectangles are in the strip's own coordinates; a painter draws in the
    // canvas's surface coordinates, whose origin is the strip's top-left.
    let (ox, oy) = (bounds.left, bounds.top);
    // The tab was sized with this padding on both sides of the title.
    let padding = PADDING.to_px(canvas.dpi()).value().max(0);
    let titles = s.titles.borrow();
    let tabs = s.tabs.borrow();
    for (index, local) in tabs.iter().enumerate() {
        let rect = local.offset(ox, oy);
        let selected = index == s.selected.get();
        if selected {
            look::face(canvas, rect, 0.0, theme.raised, &theme);
        } else if s.hover.get() == Some(index) {
            canvas.fill_rect(rect, theme.hover);
        }
        let color = if selected {
            theme.text
        } else {
            theme.text_secondary
        };
        let style = TextStyle::new(color, TEXT).middle();
        let text = Rect::new(
            rect.left + padding,
            rect.top,
            rect.right,
            (rect.bottom - ACCENT).max(rect.top),
        );
        canvas.draw_text(&titles[index], text, &style);
        if selected {
            let (left, right) = s.underline.edges(now);
            canvas.fill_rect(
                Rect::new(left + ox, rect.bottom - ACCENT, right + ox, rect.bottom),
                theme.accent,
            );
        }
    }
    drop(tabs);
    drop(titles);
    canvas.draw_line(
        Point::new(bounds.left, bounds.bottom - 1),
        Point::new(bounds.right, bounds.bottom - 1),
        theme.border,
        1.0,
    );
}

/// Handles strip input: hover highlights, a click selects.
pub(super) fn strip_event<M: 'static>(s: &Rc<Shared<M>>, ui: &Ui<M>, event: &Event) -> Option<M> {
    if ui.is_design_mode() && event.is_input() {
        return None;
    }
    match event {
        Event::MouseMove { x, .. } => {
            let hover = tab_at(s, *x);
            if s.hover.get() != hover {
                s.hover.set(hover);
                ui.invalidate(s.strip_id);
            }
        }
        Event::MouseLeave => {
            if s.hover.replace(None).is_some() {
                ui.invalidate(s.strip_id);
            }
        }
        Event::MouseUp {
            x,
            button: MouseButton::Left,
            ..
        } => {
            if let Some(index) = tab_at(s, *x) {
                choose(ui, s, index);
            }
        }
        _ => {}
    }
    None
}
