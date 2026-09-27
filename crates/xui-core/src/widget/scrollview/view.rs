#![forbid(unsafe_code)]

//! The scroll view's viewport painter and input handling. The bar itself is the
//! shared [`scrollbar`](crate::widget::scrollbar) component.

use super::{LINE_STEP, Shared, WHEEL_STEP, relayout};
use crate::app::Ui;
use crate::backend::{Canvas, Event};
use crate::message::Key;
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
    s.bar
        .handle(ui, s.metrics(), |value| set_offset(ui, s, value), event);
    None
}
