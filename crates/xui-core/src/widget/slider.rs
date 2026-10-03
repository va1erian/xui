#![forbid(unsafe_code)]

//! [`Slider`]: a themed horizontal range control.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use super::control::Control;
use crate::app::Ui;
use crate::backend::{Event, NodeKind, NodeSpec, Result};
use crate::geometry::{Point, Rect};
use crate::message::{Key, MouseButton};
use crate::property::{Properties, Property, Value};
use crate::theme::look::{self, backdrop};
use crate::units::Dip;

/// A shared, replaceable value-to-message mapper.
type MapperCell<M> = Rc<RefCell<Option<Box<dyn Fn(f64) -> Option<M>>>>>;

/// The thumb radius, as a design value.
const THUMB: Dip = Dip(8.0);
/// The track thickness, as a design value.
const TRACK: Dip = Dip(4.0);

/// A horizontal range control.
pub struct Slider<M: 'static> {
    control: Control<M>,
    value: Rc<Cell<f64>>,
    min: Rc<Cell<f64>>,
    max: Rc<Cell<f64>>,
    on_change: MapperCell<M>,
    on_commit: MapperCell<M>,
}

impl<M: 'static> Slider<M> {
    /// Creates a slider with no bounds of its own, for a layout to place (see
    /// [`crate::arrange`]); its size comes from [`Placeable`](super::Placeable).
    pub fn auto(ui: &Ui<M>, min: f64, max: f64) -> Result<Slider<M>> {
        Slider::new(ui, Rect::default(), min, max)
    }

    /// Creates a slider for `min..=max`, at its minimum, at `bounds`.
    pub fn new(ui: &Ui<M>, bounds: Rect, min: f64, max: f64) -> Result<Slider<M>> {
        let control = Control::new(ui, &NodeSpec::new(NodeKind::Slider, bounds).tab_stop())?;
        let (min, max) = if min <= max { (min, max) } else { (max, min) };
        let value = Rc::new(Cell::new(min));
        let min = Rc::new(Cell::new(min));
        let max = Rc::new(Cell::new(max));
        let dragging = Rc::new(Cell::new(false));
        let on_change: MapperCell<M> = Rc::new(RefCell::new(None));
        let on_commit: MapperCell<M> = Rc::new(RefCell::new(None));

        {
            let value = Rc::clone(&value);
            let min = Rc::clone(&min);
            let max = Rc::clone(&max);
            let theme = ui.theme_handle();
            let selected = control.selected_handle();
            control.set_painter(Rc::new(move |canvas| {
                let theme = theme.get();
                let bounds = canvas.bounds();
                let dpi = canvas.dpi();
                backdrop(canvas, theme.background);

                let track = TRACK.to_px(dpi).value().max(1);
                let thumb = THUMB.to_px(dpi).value().max(1);
                let mid = bounds.top + bounds.height() / 2;
                let (left, right) = (bounds.left + thumb, bounds.right - thumb);
                let span = (right - left).max(1);

                let range = (max.get() - min.get()).max(f64::EPSILON);
                let fraction = ((value.get() - min.get()) / range).clamp(0.0, 1.0);
                let x = left + (span as f64 * fraction).round() as i32;

                let full = Rect::new(left, mid - track / 2, right, mid - track / 2 + track);
                canvas.fill_rounded_rect(full, track as f32 / 2.0, theme.track);
                let filled = Rect::new(left, mid - track / 2, x, mid - track / 2 + track);
                look::face(canvas, filled, track as f32 / 2.0, theme.accent, &theme);
                let knob = Point::new(x, mid);
                look::glow(canvas, knob, thumb as f32, &theme);
                canvas.fill_ellipse(knob, thumb as f32, thumb as f32, theme.accent);
                if look::decorated(&theme) {
                    let core = (thumb as f32 * 0.45).max(2.0);
                    canvas.fill_ellipse(knob, core, core, theme.text_on_accent);
                }

                if selected.get() {
                    canvas.stroke_rect(bounds, theme.accent, 2.0);
                }
            }));
        }

        {
            let value = Rc::clone(&value);
            let min = Rc::clone(&min);
            let max = Rc::clone(&max);
            let dragging = Rc::clone(&dragging);
            let on_change = Rc::clone(&on_change);
            let on_commit = Rc::clone(&on_commit);
            let ui = ui.clone();
            let id = control.id();
            control.on_events(move |event| {
                if ui.is_design_mode() && event.is_input() {
                    return None;
                }
                let bounds = ui.bounds(id);
                let set_from_x = |x: i32| {
                    let dpi = ui.dpi();
                    let thumb = THUMB.to_px(dpi).value().max(1);
                    let left = thumb;
                    let right = bounds.width() - thumb;
                    let span = (right - left).max(1) as f64;
                    let fraction = ((x - left) as f64 / span).clamp(0.0, 1.0);
                    let new = min.get() + fraction * (max.get() - min.get());
                    if (new - value.get()).abs() > f64::EPSILON {
                        value.set(new);
                        return true;
                    }
                    false
                };
                let mut commit = false;
                let changed = match event {
                    Event::MouseDown {
                        x,
                        button: MouseButton::Left,
                        ..
                    } => {
                        dragging.set(true);
                        ui.set_capture(id);
                        set_from_x(*x)
                    }
                    Event::MouseMove { x, .. } if dragging.get() => set_from_x(*x),
                    Event::MouseUp {
                        button: MouseButton::Left,
                        ..
                    } => {
                        dragging.set(false);
                        ui.release_capture();
                        commit = true;
                        false
                    }
                    Event::MouseLeave if dragging.get() => {
                        dragging.set(false);
                        ui.release_capture();
                        true
                    }
                    Event::KeyDown {
                        key,
                        repeat,
                        system,
                        ..
                    } if *repeat <= 1 && !*system => {
                        let step = (max.get() - min.get()) / 20.0;
                        let new = match *key {
                            Key::LEFT | Key::DOWN => value.get() - step,
                            Key::RIGHT | Key::UP => value.get() + step,
                            Key::HOME => min.get(),
                            Key::END => max.get(),
                            _ => return None,
                        }
                        .clamp(min.get(), max.get());
                        let changed = (new - value.get()).abs() > f64::EPSILON;
                        value.set(new);
                        commit = true;
                        changed
                    }
                    _ => return None,
                };
                if changed || commit {
                    ui.invalidate(id);
                }
                // A release or a key is a commit; a drag move is a change.
                if commit && let Some(mapper) = on_commit.borrow().as_ref() {
                    return mapper(value.get());
                }
                if changed && let Some(mapper) = on_change.borrow().as_ref() {
                    return mapper(value.get());
                }
                None
            });
        }

        Ok(Slider {
            control,
            value,
            min,
            max,
            on_change,
            on_commit,
        })
    }

    /// Maps a value change to the app's message (while dragging or stepping).
    pub fn on_change(self, mapper: impl Fn(f64) -> Option<M> + 'static) -> Slider<M> {
        *self.on_change.borrow_mut() = Some(Box::new(mapper));
        self
    }

    /// Maps a committed value to the app's message (on release or a key).
    pub fn on_commit(self, mapper: impl Fn(f64) -> Option<M> + 'static) -> Slider<M> {
        *self.on_commit.borrow_mut() = Some(Box::new(mapper));
        self
    }

    /// The slider's node identity.
    pub fn id(&self) -> crate::backend::WidgetId {
        self.control.id()
    }

    /// The current value.
    pub fn value(&self) -> f64 {
        self.value.get()
    }

    /// Sets the value, clamped to the range, without raising an event.
    pub fn set_value(&self, value: f64) {
        self.value.set(value.clamp(self.min.get(), self.max.get()));
        self.control.invalidate();
    }

    /// Sets the range and clamps the value.
    pub fn set_range(&self, min: f64, max: f64) {
        let (min, max) = if min <= max { (min, max) } else { (max, min) };
        self.min.set(min);
        self.max.set(max);
        self.set_value(self.value.get());
    }

    /// Enables or disables the slider.
    pub fn set_enabled(&self, enabled: bool) {
        self.control.set_enabled(enabled);
    }

    /// Marks the slider selected (a form editor's selection).
    pub fn set_selected(&self, selected: bool) {
        self.control.set_selected(selected);
    }
}

impl<M: 'static> Properties for Slider<M> {
    fn properties(&self) -> Vec<Property> {
        vec![
            Property {
                name: "value",
                value: Value::Float(self.value()),
            },
            Property {
                name: "min",
                value: Value::Float(self.min.get()),
            },
            Property {
                name: "max",
                value: Value::Float(self.max.get()),
            },
        ]
    }

    fn set_property(&self, name: &str, value: Value) -> bool {
        match (name, value) {
            ("value", Value::Float(value)) => {
                self.set_value(value);
                true
            }
            ("min", Value::Float(min)) => {
                self.set_range(min, self.max.get());
                true
            }
            ("max", Value::Float(max)) => {
                self.set_range(self.min.get(), max);
                true
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use super::Slider;
    use crate::app::{App, Core, Runtime, Ui};
    use crate::backend::headless::HeadlessBackend;
    use crate::backend::{Backend, Event, PlatformSpec};
    use crate::geometry::Rect;
    use crate::message::{Modifiers, MouseButton};

    struct Noop;

    impl App for Noop {
        type Msg = ();
        fn update(&mut self, _msg: (), _ui: &mut Ui<()>) {}
    }

    #[test]
    fn a_thumb_drag_takes_pointer_capture_until_release() {
        let backend = Rc::new(HeadlessBackend::new());
        let window = backend.open_window(&PlatformSpec::new("slider")).unwrap();
        let core = Core::new(backend.clone(), window);
        let ui = Ui::new(Rc::clone(&core));
        let slider = Slider::new(&ui, Rect::new(0, 0, 100, 20), 0.0, 100.0).unwrap();
        let runtime = Runtime::primary(core, Noop);

        runtime.deliver(
            slider.id(),
            &Event::MouseDown {
                x: 10,
                y: 10,
                button: MouseButton::Left,
                modifiers: Modifiers::NONE,
            },
        );
        assert_eq!(
            backend.captured(),
            Some(slider.id()),
            "the drag captures the pointer"
        );

        runtime.deliver(
            slider.id(),
            &Event::MouseUp {
                x: 10,
                y: 10,
                button: MouseButton::Left,
                modifiers: Modifiers::NONE,
            },
        );
        assert_eq!(backend.captured(), None, "the release drops the capture");
    }
}
