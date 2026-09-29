#![forbid(unsafe_code)]

//! [`HueSlider`]: the horizontal rainbow hue control of a
//! [`ColorPanel`](super::ColorPanel).
//!
//! Drag, click or arrow-key it to pick a hue in degrees. Like
//! [`ColorField`](super::ColorField) it can share its value cell with a host so
//! one hue drives several views.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use super::super::control::Control;
use crate::app::Ui;
use crate::backend::{
    Canvas, Event, GradientStop, LinearGradient, NodeKind, NodeSpec, Result, Rgba,
};
use crate::color::Color;
use crate::geometry::{Point, Rect};
use crate::message::{Key, MouseButton};
use crate::theme::Themed;
use crate::units::Dip;

/// Maps a hue to an optional app message.
type Mapper<M> = Rc<RefCell<Option<Rc<dyn Fn(f32) -> Option<M>>>>>;

/// The handle radius, as a design value.
const HANDLE: Dip = Dip(6.0);
/// The track thickness, as a design value.
const TRACK: Dip = Dip(12.0);
/// The keyboard step, in degrees.
const STEP: f32 = 1.0;

/// The seven hue stops of the rainbow track, at their positions.
const RAINBOW: [(f32, Color); 7] = [
    (0.0 / 6.0, Color::rgb(0xFF, 0x00, 0x00)),
    (1.0 / 6.0, Color::rgb(0xFF, 0xFF, 0x00)),
    (2.0 / 6.0, Color::rgb(0x00, 0xFF, 0x00)),
    (3.0 / 6.0, Color::rgb(0x00, 0xFF, 0xFF)),
    (4.0 / 6.0, Color::rgb(0x00, 0x00, 0xFF)),
    (5.0 / 6.0, Color::rgb(0xFF, 0x00, 0xFF)),
    (1.0, Color::rgb(0xFF, 0x00, 0x00)),
];

/// A horizontal hue track with a round handle (`0..360`).
pub struct HueSlider<M: 'static> {
    control: Control<M>,
    hue: Rc<Cell<f32>>,
    dragging: Rc<Cell<bool>>,
    on_change: Mapper<M>,
    on_commit: Mapper<M>,
}

/// The rainbow shader, cached per track rectangle.
struct Track {
    rect: Rect,
    gradient: LinearGradient,
}

impl Track {
    /// Rebuilds the gradient only when `rect` changed.
    fn get(cache: &RefCell<Option<Rc<Track>>>, rect: Rect) -> Rc<Track> {
        if let Some(existing) = cache.borrow().as_ref()
            && existing.rect == rect
        {
            return Rc::clone(existing);
        }
        let stops = RAINBOW
            .iter()
            .map(|(position, color)| GradientStop::new(*position, Rgba::from(*color)))
            .collect();
        let gradient = LinearGradient::new(
            Point::new(rect.left, rect.top),
            Point::new(rect.right, rect.top),
            stops,
        );
        let track = Rc::new(Track { rect, gradient });
        *cache.borrow_mut() = Some(Rc::clone(&track));
        track
    }
}

impl<M: 'static> HueSlider<M> {
    /// Creates a slider at `bounds` set to `hue`.
    pub fn new(ui: &Ui<M>, bounds: Rect, hue: f32) -> Result<HueSlider<M>> {
        HueSlider::with_state(ui, bounds, Rc::new(Cell::new(hue)))
    }

    /// Creates a slider over a shared hue cell.
    pub fn with_state(ui: &Ui<M>, bounds: Rect, hue: Rc<Cell<f32>>) -> Result<HueSlider<M>> {
        let control = Control::new(ui, &NodeSpec::new(NodeKind::Custom, bounds).tab_stop())?;
        let dragging = Rc::new(Cell::new(false));
        let on_change: Mapper<M> = Rc::new(RefCell::new(None));
        let on_commit: Mapper<M> = Rc::new(RefCell::new(None));

        {
            let hue = Rc::clone(&hue);
            let cache = Rc::new(RefCell::new(None::<Rc<Track>>));
            let theme = ui.theme_handle();
            let selected = control.selected_handle();
            control.set_painter(Rc::new(move |canvas| {
                let theme = theme.get();
                let bounds = canvas.bounds();
                canvas.clear(theme.background);
                if bounds.is_empty() {
                    return;
                }
                let thickness = (TRACK.to_px(canvas.dpi()).value() as f32).max(2.0);
                let radius = thickness / 2.0;
                let mid = bounds.top + bounds.height() / 2;
                let track = Rect::new(
                    bounds.left,
                    mid - radius as i32,
                    bounds.right,
                    mid + radius as i32,
                );
                let shader = Track::get(&cache, track);
                canvas.fill_rounded_rect(track, radius, Color::rgb(0, 0, 0));
                canvas.fill_rect_linear(track, &shader.gradient);
                canvas.stroke_rounded_rect(track, radius, theme.input_border, 1.0);
                let x = track.left
                    + ((hue.get().rem_euclid(360.0) / 360.0).clamp(0.0, 1.0) * track.width() as f32)
                        .round() as i32;
                draw_handle(canvas, track, x, mid);
                if selected.get() {
                    canvas.stroke_rect(bounds, theme.accent, 2.0);
                }
            }));
        }

        {
            let hue = Rc::clone(&hue);
            let dragging = Rc::clone(&dragging);
            let on_change = Rc::clone(&on_change);
            let on_commit = Rc::clone(&on_commit);
            let ui = ui.clone();
            let id = control.id();
            control.on_events(move |event| {
                if ui.is_design_mode() && event.is_input() {
                    return None;
                }
                let width = ui.bounds(id).width().max(1) as f32;
                let set_from_x = |x: i32| {
                    let next = (x as f32 / width).clamp(0.0, 1.0) * 360.0;
                    if (next - hue.get()).abs() > f32::EPSILON {
                        hue.set(next);
                        true
                    } else {
                        false
                    }
                };
                let mut commit = false;
                let changed = match event {
                    Event::MouseDown {
                        x,
                        button: MouseButton::Left,
                        ..
                    } => {
                        ui.focus(id);
                        dragging.set(true);
                        ui.set_capture(id);
                        set_from_x(*x)
                    }
                    Event::MouseMove { x, .. } if dragging.get() => set_from_x(*x),
                    Event::MouseUp {
                        button: MouseButton::Left,
                        ..
                    } if dragging.get() => {
                        dragging.set(false);
                        ui.release_capture();
                        commit = true;
                        false
                    }
                    Event::MouseLeave | Event::CaptureChanged if dragging.get() => {
                        dragging.set(false);
                        ui.release_capture();
                        false
                    }
                    Event::KeyDown {
                        key,
                        modifiers,
                        repeat,
                        system,
                    } if *repeat <= 1 && !*system => {
                        let step = if modifiers.shift { STEP * 10.0 } else { STEP };
                        let next = match *key {
                            Key::LEFT | Key::DOWN => hue.get() - step,
                            Key::RIGHT | Key::UP => hue.get() + step,
                            Key::HOME => 0.0,
                            Key::END => 360.0,
                            _ => return None,
                        }
                        .clamp(0.0, 360.0);
                        let changed = (next - hue.get()).abs() > f32::EPSILON;
                        hue.set(next);
                        commit = true;
                        changed
                    }
                    _ => return None,
                };
                if changed || commit {
                    ui.invalidate(id);
                }
                if commit && let Some(mapper) = on_commit.borrow().clone() {
                    return mapper(hue.get());
                }
                if changed && let Some(mapper) = on_change.borrow().clone() {
                    return mapper(hue.get());
                }
                None
            });
        }

        Ok(HueSlider {
            control,
            hue,
            dragging,
            on_change,
            on_commit,
        })
    }

    /// Maps a hue changed while dragging or nudging to the app's message.
    pub fn on_change(self, mapper: impl Fn(f32) -> Option<M> + 'static) -> HueSlider<M> {
        *self.on_change.borrow_mut() = Some(Rc::new(mapper));
        self
    }

    /// Maps a committed hue (mouse-up or a key) to the app's message.
    pub fn on_commit(self, mapper: impl Fn(f32) -> Option<M> + 'static) -> HueSlider<M> {
        *self.on_commit.borrow_mut() = Some(Rc::new(mapper));
        self
    }

    /// The slider's node identity.
    pub fn id(&self) -> crate::backend::WidgetId {
        self.control.id()
    }

    /// The current hue in degrees.
    pub fn hue(&self) -> f32 {
        self.hue.get()
    }

    /// Replaces the hue without raising an event.
    pub fn set_hue(&self, hue: f32) {
        self.hue.set(hue.clamp(0.0, 360.0));
        self.control.invalidate();
    }

    /// A handle to whether a drag is in progress.
    pub(crate) fn dragging(&self) -> Rc<Cell<bool>> {
        Rc::clone(&self.dragging)
    }

    /// Moves/resizes the slider.
    pub fn set_bounds(&self, bounds: Rect) {
        self.control.set_bounds(bounds);
    }

    /// Shows or hides the slider, ending a drag in progress.
    pub fn set_visible(&self, visible: bool) {
        if !visible && self.dragging.get() {
            self.dragging.set(false);
            self.control.ui().release_capture();
        }
        self.control.set_visible(visible);
    }

    /// Enables or disables the slider, ending a drag in progress.
    pub fn set_enabled(&self, enabled: bool) {
        if !enabled && self.dragging.get() {
            self.dragging.set(false);
            self.control.ui().release_capture();
        }
        self.control.set_enabled(enabled);
    }

    /// Gives the slider the keyboard focus.
    pub fn focus(&self) {
        self.control.focus();
    }
}

impl<M: 'static> Drop for HueSlider<M> {
    fn drop(&mut self) {
        if self.dragging.replace(false) {
            self.control.ui().release_capture();
        }
    }
}

impl<M: 'static> Themed for HueSlider<M> {
    fn apply_theme(&self, _theme: &crate::theme::Theme) {
        self.control.invalidate();
    }
}

/// The white ring with a dark outline centred at `(x, y)`.
fn draw_handle(canvas: &mut dyn Canvas, rect: Rect, x: i32, y: i32) {
    let radius = (HANDLE.to_px(canvas.dpi()).value() as f32).max(2.0);
    let center = Point::new(x.clamp(rect.left, rect.right), y);
    canvas.fill_ellipse(center, radius + 1.0, radius + 1.0, Color::rgb(0, 0, 0));
    canvas.fill_ellipse(center, radius, radius, Color::rgb(255, 255, 255));
}
