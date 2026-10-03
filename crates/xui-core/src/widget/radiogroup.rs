#![forbid(unsafe_code)]

//! [`RadioGroup`]: a vertical set of themed radio options.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use super::control::Control;
use crate::app::Ui;
use crate::backend::{Event, NodeKind, NodeSpec, Result, TextStyle, WidgetId};
use crate::geometry::{Point, Rect};
use crate::message::{Key, MouseButton};
use crate::property::{Properties, Property, Value};
use crate::theme::look::{self, backdrop};
use crate::units::Dip;

/// Maps a newly selected index to an optional app message.
type SelectMapper<M> = Rc<RefCell<Option<Box<dyn Fn(usize) -> Option<M>>>>>;

/// The design height of one option.
const ROW: Dip = Dip(28.0);
/// The radius of an option's ring.
const RADIUS: Dip = Dip(8.0);
/// The gap between the ring and the label.
const GAP: Dip = Dip(8.0);
/// The design size of the label text.
const TEXT_SIZE: Dip = Dip(12.0);

/// A vertical radio group; selecting one option deselects the rest.
pub struct RadioGroup<M: 'static> {
    options: Vec<Control<M>>,
    selected: Rc<Cell<usize>>,
    enabled: Rc<Cell<bool>>,
    on_select: SelectMapper<M>,
}

impl<M: 'static> RadioGroup<M> {
    /// Creates a group of `labels`, the first selected, laid out top-to-bottom
    /// from `bounds` (each option is [`ROW`] tall).
    pub fn new(ui: &Ui<M>, bounds: Rect, labels: &[&str]) -> Result<RadioGroup<M>> {
        let selected = Rc::new(Cell::new(0usize));
        let selected_flag = Rc::new(Cell::new(false));
        let enabled = Rc::new(Cell::new(true));
        let theme = ui.theme_handle();
        let row = ROW.to_px(ui.dpi()).value();
        let mut options = Vec::with_capacity(labels.len());

        for (index, label) in labels.iter().enumerate() {
            let option = Rect::new(
                bounds.left,
                bounds.top + row * index as i32,
                bounds.right,
                bounds.top + row * (index as i32 + 1),
            );
            let control = Control::new(
                ui,
                &NodeSpec::new(NodeKind::Radio, option)
                    .text(*label)
                    .tab_stop(),
            )?;
            let selected = Rc::clone(&selected);
            let enabled_paint = Rc::clone(&enabled);
            let theme = Rc::clone(&theme);
            let selected_flag = Rc::clone(&selected_flag);
            let text = label.to_string();
            control.set_painter(Rc::new(move |canvas| {
                let theme = theme.get();
                let bounds = canvas.bounds();
                let dpi = canvas.dpi();
                let enabled = enabled_paint.get();
                backdrop(canvas, theme.background);

                let radius = RADIUS.to_px(dpi).value();
                let gap = GAP.to_px(dpi).value();
                let mid = bounds.top + bounds.height() / 2;
                let center = Point::new(bounds.left + radius, mid);
                let on = selected.get() == index;
                if look::decorated(&theme) && enabled {
                    // A well when off; a glowing accent ring around a dot when on.
                    let r = radius as f32;
                    if on {
                        look::glow(canvas, center, r, &theme);
                        canvas.fill_ellipse(center, r, r, theme.accent);
                        let dot = (r * 0.4).max(2.0);
                        canvas.fill_ellipse(center, dot, dot, theme.input_background);
                    } else {
                        canvas.fill_ellipse(center, r, r, theme.input_background);
                        canvas.stroke_ellipse(center, r, r, theme.input_border, 1.0);
                    }
                } else {
                    canvas.stroke_ellipse(
                        center,
                        radius as f32,
                        radius as f32,
                        if enabled {
                            theme.border
                        } else {
                            theme.text_disabled
                        },
                        1.0,
                    );
                    if on {
                        let inner = (radius - 3).max(2) as f32;
                        canvas.fill_ellipse(center, inner, inner, theme.accent);
                    }
                }

                let text_rect = Rect::new(
                    bounds.left + radius * 2 + gap,
                    bounds.top,
                    bounds.right,
                    bounds.bottom,
                );
                let color = if enabled {
                    theme.text
                } else {
                    theme.text_disabled
                };
                let style = TextStyle::new(color, TEXT_SIZE).middle();
                canvas.draw_text(&text, text_rect, &style);

                if selected_flag.get() {
                    canvas.stroke_rect(bounds, theme.accent, 2.0);
                }
            }));
            options.push(control);
        }

        let ids: Vec<WidgetId> = options.iter().map(Control::id).collect();
        let on_select: SelectMapper<M> = Rc::new(RefCell::new(None));
        for (index, control) in options.iter().enumerate() {
            let selected = Rc::clone(&selected);
            let enabled = Rc::clone(&enabled);
            let on_select = Rc::clone(&on_select);
            let ids = ids.clone();
            let ui = ui.clone();
            control.on_events(move |event| {
                if ui.is_design_mode() && event.is_input() {
                    return None;
                }
                if !enabled.get() {
                    return None;
                }
                let activate = match event {
                    Event::MouseUp {
                        button: MouseButton::Left,
                        ..
                    } => true,
                    Event::KeyDown {
                        key: Key::SPACE,
                        repeat,
                        system,
                        ..
                    } if *repeat <= 1 && !*system => true,
                    _ => return None,
                };
                if !activate {
                    return None;
                }
                if selected.get() != index {
                    selected.set(index);
                    for id in &ids {
                        ui.invalidate(*id);
                    }
                }
                let mapper = on_select.borrow();
                if let Some(mapper) = mapper.as_ref() {
                    return mapper(index);
                }
                None
            });
        }

        Ok(RadioGroup {
            options,
            selected,
            enabled,
            on_select,
        })
    }

    /// Maps a selection to the app's message: the closure receives the selected
    /// index and returns `Some(msg)` to raise it, or `None` to ignore it.
    pub fn on_select(self, mapper: impl Fn(usize) -> Option<M> + 'static) -> RadioGroup<M> {
        *self.on_select.borrow_mut() = Some(Box::new(mapper));
        self
    }

    /// The selected index.
    pub fn selected(&self) -> usize {
        self.selected.get()
    }

    /// Selects `index` without raising the event.
    pub fn select(&self, index: usize) {
        if index < self.options.len() {
            self.selected.set(index);
            for option in &self.options {
                option.invalidate();
            }
        }
    }

    /// The option node identities, in order.
    pub fn ids(&self) -> Vec<WidgetId> {
        self.options.iter().map(Control::id).collect()
    }

    /// Enables or disables the group. A disabled group is dimmed and ignores
    /// input.
    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.set(enabled);
        for option in &self.options {
            option.set_enabled(enabled);
            option.invalidate();
        }
    }

    /// Marks every option selected (a form editor's selection).
    pub fn set_selected(&self, selected: bool) {
        for option in &self.options {
            option.set_selected(selected);
        }
    }
}

impl<M: 'static> Properties for RadioGroup<M> {
    fn properties(&self) -> Vec<Property> {
        vec![Property {
            name: "selected",
            value: Value::Integer(self.selected() as i64),
        }]
    }

    fn set_property(&self, name: &str, value: Value) -> bool {
        match (name, value) {
            ("selected", Value::Integer(index)) if index >= 0 => {
                self.select(index as usize);
                true
            }
            _ => false,
        }
    }
}
