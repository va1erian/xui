#![forbid(unsafe_code)]

//! [`ColorField`]: the saturation/value square of a [`ColorPanel`](super::ColorPanel).
//!
//! It paints the field for the current hue and maps a drag, click or keyboard
//! nudge to the app's message. The widget owns the [`Hsv`] triple it edits, so
//! a host can share that state (see [`ColorField::with_state`]) and keep several
//! views in sync from one source of truth.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use super::super::control::Control;
use super::model::Hsv;
use crate::app::Ui;
use crate::backend::{
    Canvas, Event, GradientStop, LinearGradient, NodeKind, NodeSpec, Result, Rgba,
};
use crate::color::Color;
use crate::geometry::{Point, Rect};
use crate::message::{Key, MouseButton};
use crate::theme::Themed;
use crate::theme::look::backdrop;
use crate::units::Dip;

/// Maps an edited triple to an optional app message.
type Mapper<M> = Rc<RefCell<Option<Rc<dyn Fn(Hsv) -> Option<M>>>>>;

/// The handle radius, as a design value.
const HANDLE: Dip = Dip(6.0);
/// The keyboard nudge, as a fraction of the field.
const NUDGE: f32 = 0.01;

/// The saturation/value square for one hue.
pub struct ColorField<M: 'static> {
    control: Control<M>,
    hsv: Rc<Cell<Hsv>>,
    dragging: Rc<Cell<bool>>,
    on_change: Mapper<M>,
    on_commit: Mapper<M>,
}

/// The two overlays that turn a solid hue into the SV square, cached per size.
struct Gradients {
    rect: Rect,
    white: LinearGradient,
    black: LinearGradient,
}

impl Gradients {
    /// Rebuilds the overlays only when `rect` changed.
    fn get(cache: &RefCell<Option<Rc<Gradients>>>, rect: Rect) -> Rc<Gradients> {
        if let Some(existing) = cache.borrow().as_ref()
            && existing.rect == rect
        {
            return Rc::clone(existing);
        }
        let white = LinearGradient::new(
            Point::new(rect.left, rect.top),
            Point::new(rect.right, rect.top),
            vec![
                GradientStop::new(0.0, Rgba::rgb(0xFF, 0xFF, 0xFF)),
                GradientStop::new(1.0, Rgba::with_alpha(0xFF, 0xFF, 0xFF, 0x00)),
            ],
        );
        let black = LinearGradient::new(
            Point::new(rect.left, rect.top),
            Point::new(rect.left, rect.bottom),
            vec![
                GradientStop::new(0.0, Rgba::with_alpha(0x00, 0x00, 0x00, 0x00)),
                GradientStop::new(1.0, Rgba::rgb(0x00, 0x00, 0x00)),
            ],
        );
        let gradients = Rc::new(Gradients { rect, white, black });
        *cache.borrow_mut() = Some(Rc::clone(&gradients));
        gradients
    }
}

impl<M: 'static> ColorField<M> {
    /// Creates a field at `bounds` for `hsv`.
    pub fn new(ui: &Ui<M>, bounds: Rect, hsv: Hsv) -> Result<ColorField<M>> {
        ColorField::with_state(ui, bounds, Rc::new(Cell::new(hsv)))
    }

    /// Creates a field over a shared [`Hsv`] cell, so a host keeps one source
    /// of truth for the field, a preview and the text boxes.
    pub(crate) fn with_state(
        ui: &Ui<M>,
        bounds: Rect,
        hsv: Rc<Cell<Hsv>>,
    ) -> Result<ColorField<M>> {
        let control = Control::new(ui, &NodeSpec::new(NodeKind::Custom, bounds).tab_stop())?;
        let dragging = Rc::new(Cell::new(false));
        let on_change: Mapper<M> = Rc::new(RefCell::new(None));
        let on_commit: Mapper<M> = Rc::new(RefCell::new(None));

        {
            let hsv = Rc::clone(&hsv);
            let cache = Rc::new(RefCell::new(None::<Rc<Gradients>>));
            let theme = ui.theme_handle();
            let selected = control.selected_handle();
            control.set_painter(Rc::new(move |canvas| {
                let theme = theme.get();
                let bounds = canvas.bounds();
                backdrop(canvas, theme.background);
                if bounds.is_empty() {
                    return;
                }
                let triples = hsv.get();
                let base = Hsv::new(triples.h, 1.0, 1.0).to_color();
                canvas.fill_rect(bounds, base);
                let gradients = Gradients::get(&cache, bounds);
                canvas.fill_rect_linear(bounds, &gradients.white);
                canvas.fill_rect_linear(bounds, &gradients.black);
                canvas.stroke_rect(bounds, theme.input_border, 1.0);
                draw_handle(canvas, bounds, triples.s, 1.0 - triples.v);
                if selected.get() {
                    canvas.stroke_rect(bounds, theme.accent, 2.0);
                }
            }));
        }

        {
            let hsv = Rc::clone(&hsv);
            let dragging = Rc::clone(&dragging);
            let on_change = Rc::clone(&on_change);
            let on_commit = Rc::clone(&on_commit);
            let ui = ui.clone();
            let id = control.id();
            control.on_events(move |event| {
                if ui.is_design_mode() && event.is_input() {
                    return None;
                }
                let size = ui.bounds(id);
                let width = size.width().max(1) as f32;
                let height = size.height().max(1) as f32;
                let set_from_point = |x: i32, y: i32| {
                    let old = hsv.get();
                    let s = (x as f32 / width).clamp(0.0, 1.0);
                    let v = (1.0 - y as f32 / height).clamp(0.0, 1.0);
                    let new = Hsv { h: old.h, s, v };
                    if new != old {
                        hsv.set(new);
                        true
                    } else {
                        false
                    }
                };
                let mut commit = false;
                let changed = match event {
                    Event::MouseDown {
                        x,
                        y,
                        button: MouseButton::Left,
                        ..
                    } => {
                        ui.focus(id);
                        dragging.set(true);
                        ui.set_capture(id);
                        set_from_point(*x, *y)
                    }
                    Event::MouseMove { x, y, .. } if dragging.get() => set_from_point(*x, *y),
                    Event::MouseUp {
                        button: MouseButton::Left,
                        ..
                    } if dragging.get() => {
                        dragging.set(false);
                        ui.release_capture();
                        commit = true;
                        false
                    }
                    Event::CaptureChanged if dragging.get() => {
                        dragging.set(false);
                        commit = true;
                        false
                    }
                    Event::KeyDown {
                        key,
                        modifiers,
                        repeat,
                        system,
                    } if *repeat <= 1 && !*system => {
                        let old = hsv.get();
                        let step = if modifiers.shift { NUDGE * 10.0 } else { NUDGE };
                        let new = match *key {
                            Key::LEFT => Hsv {
                                s: old.s - step,
                                ..old
                            },
                            Key::RIGHT => Hsv {
                                s: old.s + step,
                                ..old
                            },
                            Key::UP => Hsv {
                                v: old.v + step,
                                ..old
                            },
                            Key::DOWN => Hsv {
                                v: old.v - step,
                                ..old
                            },
                            _ => return None,
                        };
                        let new = Hsv::new(new.h, new.s, new.v);
                        let changed = new != old;
                        hsv.set(new);
                        commit = true;
                        changed
                    }
                    _ => return None,
                };
                if changed || commit {
                    ui.invalidate(id);
                }
                if commit && let Some(mapper) = on_commit.borrow().clone() {
                    return mapper(hsv.get());
                }
                if changed && let Some(mapper) = on_change.borrow().clone() {
                    return mapper(hsv.get());
                }
                None
            });
        }

        Ok(ColorField {
            control,
            hsv,
            dragging,
            on_change,
            on_commit,
        })
    }

    /// Maps a change while dragging or nudging to the app's message.
    pub fn on_change(self, mapper: impl Fn(Hsv) -> Option<M> + 'static) -> ColorField<M> {
        *self.on_change.borrow_mut() = Some(Rc::new(mapper));
        self
    }

    /// Maps a committed colour (mouse-up or a key) to the app's message.
    pub fn on_commit(self, mapper: impl Fn(Hsv) -> Option<M> + 'static) -> ColorField<M> {
        *self.on_commit.borrow_mut() = Some(Rc::new(mapper));
        self
    }

    /// The field's node identity.
    pub fn id(&self) -> crate::backend::WidgetId {
        self.control.id()
    }

    /// The current triple.
    pub fn hsv(&self) -> Hsv {
        self.hsv.get()
    }

    /// Replaces the triple without raising an event.
    pub fn set_hsv(&self, hsv: Hsv) {
        self.hsv.set(hsv);
        self.control.invalidate();
    }

    /// A handle to whether a drag is in progress, so a host can end one when it
    /// hides or disables the field.
    pub(crate) fn dragging(&self) -> Rc<Cell<bool>> {
        Rc::clone(&self.dragging)
    }

    /// Moves/resizes the field.
    pub fn set_bounds(&self, bounds: Rect) {
        self.control.set_bounds(bounds);
    }

    /// Shows or hides the field, ending a drag in progress.
    pub fn set_visible(&self, visible: bool) {
        if !visible && self.dragging.get() {
            self.dragging.set(false);
            self.control.ui().release_capture();
        }
        self.control.set_visible(visible);
    }

    /// Enables or disables the field, ending a drag in progress.
    pub fn set_enabled(&self, enabled: bool) {
        if !enabled && self.dragging.get() {
            self.dragging.set(false);
            self.control.ui().release_capture();
        }
        self.control.set_enabled(enabled);
    }

    /// Gives the field the keyboard focus.
    pub fn focus(&self) {
        self.control.focus();
    }
}

impl<M: 'static> Drop for ColorField<M> {
    fn drop(&mut self) {
        // A drag must not outlive the widget: release the pointer capture so a
        // later move does not route to the node being destroyed.
        if self.dragging.replace(false) {
            self.control.ui().release_capture();
        }
    }
}

impl<M: 'static> Themed for ColorField<M> {
    fn apply_theme(&self, _theme: &crate::theme::Theme) {
        self.control.invalidate();
    }
}

/// The white ring with a dark outline at `(s, down)`, where `down` is measured
/// from the top.
fn draw_handle(canvas: &mut dyn Canvas, rect: Rect, s: f32, down: f32) {
    let radius = (HANDLE.to_px(canvas.dpi()).value() as f32).max(2.0);
    let x = rect.left + (s.clamp(0.0, 1.0) * rect.width() as f32).round() as i32;
    let y = rect.top + (down.clamp(0.0, 1.0) * rect.height() as f32).round() as i32;
    let center = Point::new(
        x.clamp(rect.left, rect.right),
        y.clamp(rect.top, rect.bottom),
    );
    canvas.fill_ellipse(center, radius + 1.0, radius + 1.0, Color::rgb(0, 0, 0));
    canvas.fill_ellipse(center, radius, radius, Color::rgb(255, 255, 255));
}
