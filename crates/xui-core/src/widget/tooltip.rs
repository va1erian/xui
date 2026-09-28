#![forbid(unsafe_code)]

//! [`Tooltip`]: a portable hover tooltip.
//!
//! A tooltip is a painted popup node. It appears after the pointer has rested
//! on a widget for [`Tooltip::delay`] milliseconds, positioned under the
//! widget, and is dismissed when the pointer leaves it or a button goes down.
//!
//! [`Tooltip::attach`] installs it on an existing widget's node without
//! touching the widget: it *adds* a listener to the node's events (the widget
//! keeps handling its own input) and a listener for the delay timer. The popup
//! is a sibling of the target, so every widget can carry one.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use super::control::{Control, HasText};
use super::popup;
use crate::app::Ui;
use crate::backend::{Canvas, Event, NodeKind, NodeSpec, Result, TextStyle, TimerId, WidgetId};
use crate::geometry::Rect;
use crate::theme::Theme;
use crate::units::Dip;

/// The tooltip text size, as a design value.
const TEXT_SIZE: Dip = Dip(12.0);
/// Padding between the text and the popup edge.
const PADDING: Dip = Dip(8.0);
/// Gap between the target and the popup.
const GAP: Dip = Dip(4.0);
/// The hover delay before the popup appears.
const DEFAULT_DELAY_MS: u32 = 500;

/// The state the popup painter and the target/timer listeners share.
struct Shared {
    /// The tip text.
    text: RefCell<String>,
    /// The pending show timer while the pointer rests on the target.
    pending: Cell<Option<TimerId>>,
    /// Whether the popup is on screen.
    visible: Cell<bool>,
    /// Whether a button is held on the target, which suppresses the tip.
    pressed: Cell<bool>,
    /// The hover delay in milliseconds.
    delay: Cell<u32>,
}

/// A portable hover tooltip attached to a widget's node.
pub struct Tooltip<M: 'static> {
    ui: Ui<M>,
    popup: Control<M>,
    target: WidgetId,
    shared: Rc<Shared>,
    event_listener: usize,
    timer_listener: usize,
}

impl<M: 'static> Tooltip<M> {
    /// Attaches a tooltip reading `text` to the widget owning `target`.
    ///
    /// The target keeps its own event handling; the tooltip only observes it.
    /// Drive the widget as usual and drop the returned handle to remove the
    /// tooltip.
    pub fn attach(ui: &Ui<M>, target: WidgetId, text: &str) -> Result<Tooltip<M>> {
        let shared = Rc::new(Shared {
            text: RefCell::new(text.to_string()),
            pending: Cell::new(None),
            visible: Cell::new(false),
            pressed: Cell::new(false),
            delay: Cell::new(DEFAULT_DELAY_MS),
        });
        let spec = NodeSpec::new(NodeKind::Custom, Rect::new(0, 0, 0, 0)).text(text);
        let popup = Control::new(ui, &spec)?;
        let popup_id = popup.id();
        ui.set_visible(popup_id, false);

        {
            let shared = Rc::clone(&shared);
            let theme = ui.theme_handle();
            popup.set_painter(Rc::new(move |canvas| paint(&shared, canvas, theme.get())));
        }

        let event_listener = {
            let shared = Rc::clone(&shared);
            let listener_ui = ui.clone();
            ui.add_events(target, move |event| {
                target_event(&listener_ui, popup_id, &shared, event);
                None
            })
        };
        let timer_listener = {
            let shared = Rc::clone(&shared);
            let listener_ui = ui.clone();
            ui.add_timer_listener(move |fired| {
                show(&listener_ui, target, popup_id, &shared, fired);
                None
            })
        };

        Ok(Tooltip {
            ui: ui.clone(),
            popup,
            target,
            shared,
            event_listener,
            timer_listener,
        })
    }

    /// Sets the hover delay before the popup appears, in milliseconds.
    pub fn delay(self, millis: u32) -> Tooltip<M> {
        self.shared.delay.set(millis);
        self
    }

    /// The tooltip popup's node identity.
    pub fn id(&self) -> WidgetId {
        self.popup.id()
    }

    /// The node the tooltip is attached to.
    pub fn target(&self) -> WidgetId {
        self.target
    }

    /// Whether the popup is currently on screen.
    pub fn is_visible(&self) -> bool {
        self.shared.visible.get()
    }

    /// Shows the popup immediately, as if the hover delay had elapsed.
    ///
    /// A tooltip is normally hover-driven; this is for showing one
    /// deliberately, for example when validating a form field.
    pub fn show(&self) {
        if let Some(rect) = placed(&self.ui, self.target, &self.shared) {
            self.ui.apply_moves(&[(self.popup.id(), rect)]);
            self.ui.set_visible(self.popup.id(), true);
            self.ui.raise(self.popup.id());
            self.shared.visible.set(true);
        }
    }

    /// Hides the popup and cancels a pending hover delay.
    pub fn hide(&self) {
        hide(&self.ui, self.popup.id(), &self.shared);
    }

    /// Replaces the tip text, resizing the popup if it is on screen.
    pub fn set_text(&self, text: &str) {
        *self.shared.text.borrow_mut() = text.to_string();
        self.popup.set_text(text);
        if self.shared.visible.get() {
            if let Some(rect) = placed(&self.ui, self.target, &self.shared) {
                self.ui.apply_moves(&[(self.popup.id(), rect)]);
            }
            self.ui.invalidate(self.popup.id());
        }
    }

    /// The current tip text.
    pub fn text(&self) -> String {
        self.shared.text.borrow().clone()
    }
}

impl<M: 'static> Drop for Tooltip<M> {
    fn drop(&mut self) {
        // The listeners captured the `Ui` (and so the `Core`); removing them
        // breaks the cycle, exactly as `Control`'s drop does for a widget.
        self.ui.remove_events(self.target, self.event_listener);
        self.ui.remove_timer_listener(self.timer_listener);
        if let Some(timer) = self.shared.pending.take() {
            self.ui.kill_timer(timer);
        }
    }
}

impl<M: 'static> HasText for Tooltip<M> {
    fn text(&self) -> String {
        self.text()
    }

    fn set_text(&self, text: &str) {
        self.set_text(text);
    }
}

/// The popup rectangle: under the target, flipped above it when it would cross
/// the bottom of the client area, and kept inside the client horizontally.
fn placed<M: 'static>(ui: &Ui<M>, target: WidgetId, shared: &Shared) -> Option<Rect> {
    let text = shared.text.borrow();
    if text.is_empty() {
        return None;
    }
    let dpi = ui.dpi();
    let style = TextStyle::new(ui.theme().text, TEXT_SIZE);
    let metrics = ui.measure_text(text.as_str(), &style, dpi);
    let pad = PADDING.to_px(dpi).value();
    let gap = GAP.to_px(dpi).value();
    let width = (metrics.width + pad * 2).max(1);
    let height = (metrics.height + pad * 2).max(1);

    let anchor = ui.bounds(target);
    let client = ui.client_rect();
    let mut top = anchor.bottom + gap;
    if top + height > client.bottom {
        top = anchor.top - gap - height;
    }
    let left = anchor
        .left
        .clamp(client.left, (client.right - width).max(client.left));
    let top = top.clamp(client.top, (client.bottom - height).max(client.top));
    Some(Rect::new(left, top, left + width, top + height))
}

/// Shows the popup if `fired` is still the pending show timer.
fn show<M: 'static>(
    ui: &Ui<M>,
    target: WidgetId,
    popup: WidgetId,
    shared: &Shared,
    fired: TimerId,
) {
    if shared.pending.get() != Some(fired) {
        return;
    }
    shared.pending.set(None);
    // The timer repeats; the delay is a one-shot.
    ui.kill_timer(fired);
    if let Some(rect) = placed(ui, target, shared) {
        ui.apply_moves(&[(popup, rect)]);
        ui.set_visible(popup, true);
        ui.raise(popup);
        shared.visible.set(true);
    }
}

/// Cancels a pending show and hides the popup.
fn hide<M: 'static>(ui: &Ui<M>, popup: WidgetId, shared: &Shared) {
    if let Some(timer) = shared.pending.take() {
        ui.kill_timer(timer);
    }
    if shared.visible.replace(false) {
        ui.set_visible(popup, false);
    }
}

/// Observes the target's input: a resting pointer arms the delay, leaving or a
/// press dismisses the tip.
fn target_event<M: 'static>(ui: &Ui<M>, popup: WidgetId, shared: &Shared, event: &Event) {
    if ui.is_design_mode() && event.is_input() {
        return;
    }
    match event {
        Event::MouseMove { .. } => {
            if !shared.pressed.get() && !shared.visible.get() && shared.pending.get().is_none() {
                let timer = ui.set_timer(shared.delay.get());
                if timer.0 != 0 {
                    shared.pending.set(Some(timer));
                }
            }
        }
        Event::MouseDown { .. } => {
            shared.pressed.set(true);
            hide(ui, popup, shared);
        }
        Event::MouseUp { .. } => shared.pressed.set(false),
        Event::MouseLeave | Event::CaptureChanged => {
            shared.pressed.set(false);
            hide(ui, popup, shared);
        }
        _ => {}
    }
}

/// Paints the popup face and text from the theme.
fn paint(shared: &Shared, canvas: &mut dyn Canvas, theme: Theme) {
    let bounds = canvas.bounds();
    let pad = PADDING.to_px(canvas.dpi()).value();
    popup::paint(canvas, theme, theme.raised);
    let text = shared.text.borrow();
    let style = TextStyle::new(theme.text, TEXT_SIZE).middle();
    canvas.draw_text(text.as_str(), bounds.shrink(pad), &style);
}

#[cfg(test)]
mod tests;
