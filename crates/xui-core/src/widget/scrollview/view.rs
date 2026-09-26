#![forbid(unsafe_code)]

//! The scroll view's scrollbar geometry, painters and input handling.

use super::{LINE_STEP, MIN_THUMB, Shared, WHEEL_STEP, relayout};
use crate::app::Ui;
use crate::backend::{Canvas, Event};
use crate::geometry::Rect;
use crate::message::{Key, MouseButton};
use crate::theme::Theme;
use crate::units::Px;

/// Sets `value` as the offset (clamped), moves the rows and raises the event.
pub(super) fn set_offset<M>(ui: &Ui<M>, s: &Shared<M>, value: i32) {
    let max = (s.content.get() - s.viewport.get().height()).max(0);
    let value = value.clamp(0, max);
    if value == s.offset.get() {
        return;
    }
    s.offset.set(value);
    relayout(ui, s);
    let mapped = s.on_scroll.borrow().as_ref().and_then(|f| f(Px(value)));
    if let Some(msg) = mapped {
        ui.emit(msg);
    }
}

/// The thumb's rectangle within `track`, or `None` when nothing scrolls.
pub(super) fn thumb_rect<M>(s: &Shared<M>, track: Rect, dpi: u32) -> Option<Rect> {
    let viewport_height = s.viewport.get().height().max(0);
    let content = s.content.get();
    if content <= viewport_height || track.height() <= 0 {
        return None;
    }
    let min = MIN_THUMB.to_px(dpi).value().min(track.height());
    let proportional =
        (i64::from(track.height()) * i64::from(viewport_height) / i64::from(content)) as i32;
    let thumb_height = proportional.clamp(min, track.height());
    let travel = track.height() - thumb_height;
    let max_offset = (content - viewport_height).max(0);
    let y = if max_offset > 0 {
        track.top + travel * s.offset.get() / max_offset
    } else {
        track.top
    };
    Some(Rect::new(
        track.left + 2,
        y,
        track.right - 2,
        y + thumb_height,
    ))
}

/// Paints the viewport: its own drawing is clipped to it with `push_clip`.
///
/// A backend clips a node's children to it separately (Win32 child windows,
/// a compositor with per-node clips); this clip only bounds the view's own
/// background.
pub(super) fn paint_viewport(canvas: &mut dyn Canvas, theme: Theme) {
    let bounds = canvas.bounds();
    canvas.clear(theme.background);
    canvas.push_clip(bounds);
    canvas.fill_rect(bounds, theme.surface);
    canvas.pop_clip();
    canvas.stroke_rect(bounds, theme.border, 1.0);
}

/// Paints the scrollbar's track and thumb.
pub(super) fn paint_bar<M>(s: &Shared<M>, canvas: &mut dyn Canvas, theme: Theme) {
    let track = canvas.bounds();
    let dpi = canvas.dpi();
    canvas.clear(theme.background);
    canvas.fill_rect(track, theme.scrollbar_track);
    if let Some(thumb) = thumb_rect(s, track, dpi) {
        let radius = thumb.width() as f32 / 2.0;
        canvas.fill_rounded_rect(thumb, radius, theme.scrollbar);
    }
}

/// Handles the view's wheel, keyboard scroll and resize.
pub(super) fn viewport_event<M>(s: &Shared<M>, ui: &Ui<M>, event: &Event) -> Option<M> {
    if ui.is_design_mode() && event.is_input() {
        return None;
    }
    let dpi = ui.dpi();
    match event {
        Event::MouseWheel {
            delta,
            horizontal: false,
            ..
        } => {
            if *delta != 0 {
                let step = WHEEL_STEP.to_px(dpi).value();
                let target = s.offset.get() - i32::from(delta.signum()) * step;
                set_offset(ui, s, target);
            }
        }
        Event::KeyDown { key, .. } => {
            let view = s.viewport.get().height().max(0);
            let line = LINE_STEP.to_px(dpi).value();
            let target = match *key {
                Key::UP => s.offset.get() - line,
                Key::DOWN => s.offset.get() + line,
                Key::PAGE_UP => s.offset.get() - view,
                Key::PAGE_DOWN => s.offset.get() + view,
                Key::HOME => 0,
                Key::END => s.content.get(),
                _ => return None,
            };
            set_offset(ui, s, target);
        }
        Event::Resize { .. } => relayout(ui, s),
        _ => return None,
    }
    None
}

/// Handles the scrollbar: thumb drag, track paging and drag release.
pub(super) fn bar_event<M>(s: &Shared<M>, ui: &Ui<M>, event: &Event) -> Option<M> {
    if ui.is_design_mode() && event.is_input() {
        return None;
    }
    let dpi = ui.dpi();
    match event {
        Event::MouseDown {
            y,
            button: MouseButton::Left,
            ..
        } => {
            let track = s.bar.get();
            if let Some(thumb) = thumb_rect(s, track, dpi) {
                if *y >= thumb.top && *y < thumb.bottom {
                    s.drag.set(Some(*y));
                    s.drag_offset.set(s.offset.get());
                    ui.set_capture(s.bar_id);
                } else {
                    let view = s.viewport.get().height().max(1);
                    let target = if *y < thumb.top {
                        s.offset.get() - view
                    } else {
                        s.offset.get() + view
                    };
                    set_offset(ui, s, target);
                }
            }
        }
        Event::MouseMove { y, .. } => {
            if let Some(start) = s.drag.get() {
                let track = s.bar.get();
                if let Some(thumb) = thumb_rect(s, track, dpi) {
                    let travel = (track.height() - thumb.height()).max(1);
                    let max = (s.content.get() - s.viewport.get().height()).max(0);
                    let target = s.drag_offset.get() + (*y - start) * max / travel;
                    set_offset(ui, s, target);
                }
            }
        }
        Event::MouseUp {
            button: MouseButton::Left,
            ..
        }
        | Event::CaptureChanged => {
            s.drag.set(None);
            ui.release_capture();
        }
        _ => {}
    }
    None
}
