#![forbid(unsafe_code)]

//! [`Split`]: two panes separated by a draggable divider.
//!
//! The panes are lists of child widgets, created through [`Split::ui`] and
//! registered with [`Split::pane_a`]/[`Split::pane_b`]. The divider is a child
//! of the split: dragging it resizes both panes live, the arrow keys move it,
//! and [`Split::on_moved`] maps a move to the app's message.

#[cfg(test)]
mod tests;

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use super::control::Control;
use crate::app::Ui;
use crate::backend::{Canvas, Cursor, Event, NodeKind, NodeSpec, Result, WidgetId};
use crate::geometry::{Point, Rect};
use crate::layout::{Stack, StackSlot};
use crate::message::{Key, MouseButton};
use crate::property::{Properties, Property, Value};
use crate::theme::Theme;
use crate::theme::look::backdrop;
use crate::units::{Dip, Px};

/// The divider's thickness.
const DIVIDER: Dip = Dip(5.0);
/// How far an arrow key moves the divider.
const ARROW_STEP: Dip = Dip(8.0);

type MovedMapper<M> = RefCell<Option<Box<dyn Fn(Dip) -> Option<M>>>>;

/// State the container, its divider and their painters and mappers share.
struct Shared<M: 'static> {
    id: WidgetId,
    divider_id: WidgetId,
    /// Whether the panes sit side by side (a row) or stacked (a column).
    horizontal: bool,
    /// The first pane's extent in design values; `None` centres the divider.
    position: Cell<Option<Dip>>,
    min_a: Cell<Dip>,
    min_b: Cell<Dip>,
    /// The panes' children: the first (left/top) then the second.
    panes: RefCell<(Vec<WidgetId>, Vec<WidgetId>)>,
    /// The first pane's extent in pixels, as last laid out.
    current: Cell<i32>,
    /// The divider's rectangle in the split's own coordinates.
    divider: Cell<Rect>,
    /// The pointer's coordinate on the drag axis when a drag started, in the
    /// split's own coordinates: stable while the divider moves under it.
    drag: Cell<Option<i32>>,
    /// The first pane's extent in pixels when the drag started.
    drag_start: Cell<i32>,
    on_moved: MovedMapper<M>,
}

/// Two panes separated by a draggable divider.
pub struct Split<M: 'static> {
    control: Control<M>,
    _divider: Control<M>,
    scoped: Ui<M>,
    shared: Rc<Shared<M>>,
}

impl<M: 'static> Split<M> {
    /// A split whose panes sit side by side, at `bounds`.
    pub fn row(ui: &Ui<M>, bounds: Rect) -> Result<Split<M>> {
        Split::new(ui, bounds, true)
    }

    /// A split whose panes are stacked, at `bounds`.
    pub fn column(ui: &Ui<M>, bounds: Rect) -> Result<Split<M>> {
        Split::new(ui, bounds, false)
    }

    fn new(ui: &Ui<M>, bounds: Rect, horizontal: bool) -> Result<Split<M>> {
        let control = Control::new(ui, &NodeSpec::new(NodeKind::Container, bounds))?;
        let scoped = ui.with_parent(control.id());
        let divider = Control::new(
            &scoped,
            &NodeSpec::new(NodeKind::Container, Rect::default()),
        )?;
        let shared = Rc::new(Shared {
            id: control.id(),
            divider_id: divider.id(),
            horizontal,
            position: Cell::new(None),
            min_a: Cell::new(Dip(0.0)),
            min_b: Cell::new(Dip(0.0)),
            panes: RefCell::new((Vec::new(), Vec::new())),
            current: Cell::new(0),
            divider: Cell::new(Rect::default()),
            drag: Cell::new(None),
            drag_start: Cell::new(0),
            on_moved: RefCell::new(None),
        });
        {
            let theme = ui.theme_handle();
            control.set_painter(Rc::new(move |canvas| {
                backdrop(canvas, theme.get().background)
            }));
        }
        {
            let theme = ui.theme_handle();
            divider.set_painter(Rc::new(move |canvas| paint_divider(canvas, theme.get())));
        }
        // The divider shows the resize cursor that matches the drag axis.
        ui.set_cursor(
            divider.id(),
            if horizontal {
                Cursor::SizeHorizontal
            } else {
                Cursor::SizeVertical
            },
        );
        {
            let shared = Rc::clone(&shared);
            let ui = scoped.clone();
            divider.on_events(move |event| divider_event(&shared, &ui, event));
        }
        Ok(Split {
            control,
            _divider: divider,
            scoped,
            shared,
        })
    }

    /// The handle widgets built inside this split parent to.
    pub fn ui(&self) -> &Ui<M> {
        &self.scoped
    }

    /// Puts the widgets inside this split in or out of design mode: they
    /// ignore their own input while everything outside stays live. Nested
    /// containers inherit it.
    pub fn set_design_mode(&self, on: bool) {
        self.ui().set_design_mode(on);
    }

    /// The split's node identity.
    pub fn id(&self) -> WidgetId {
        self.control.id()
    }

    /// Sets the first (left/top) pane's children.
    pub fn pane_a(&self, children: &[WidgetId]) {
        self.shared.panes.borrow_mut().0 = children.to_vec();
        relayout(&self.scoped, &self.shared);
    }

    /// Sets the second (right/bottom) pane's children.
    pub fn pane_b(&self, children: &[WidgetId]) {
        self.shared.panes.borrow_mut().1 = children.to_vec();
        relayout(&self.scoped, &self.shared);
    }

    /// Sets the first pane's initial extent; the divider centres without this.
    pub fn set_position(&self, position: Dip) {
        self.shared.position.set(Some(position));
        relayout(&self.scoped, &self.shared);
    }

    /// Sets the minimum extents of the two panes.
    pub fn set_min(&self, a: Dip, b: Dip) {
        self.shared.min_a.set(a);
        self.shared.min_b.set(b);
    }

    /// The first pane's current extent.
    pub fn position(&self) -> Dip {
        Px(self.shared.current.get()).to_dip(self.control.dpi())
    }

    /// Maps a divider move to the app's message: the closure receives the first
    /// pane's extent and returns `Some(msg)` to raise it, or `None` to ignore
    /// it (the move is still applied).
    pub fn on_moved(self, f: impl Fn(Dip) -> Option<M> + 'static) -> Split<M> {
        *self.shared.on_moved.borrow_mut() = Some(Box::new(f));
        self
    }

    /// Moves/resizes the split and re-lays its panes out.
    pub fn set_bounds(&self, bounds: Rect) {
        self.control.set_bounds(bounds);
        relayout(&self.scoped, &self.shared);
    }

    /// Re-lays the panes out from the split's current bounds.
    pub fn relayout(&self) {
        relayout(&self.scoped, &self.shared);
    }

    /// Shows or hides the split.
    pub fn set_visible(&self, visible: bool) {
        self.control.set_visible(visible);
    }

    /// Enables or disables the split.
    pub fn set_enabled(&self, enabled: bool) {
        self.control.set_enabled(enabled);
    }
}

impl<M: 'static> Properties for Split<M> {
    fn properties(&self) -> Vec<Property> {
        vec![Property {
            name: "position",
            value: Value::Float(f64::from(self.position().value())),
        }]
    }

    fn set_property(&self, name: &str, value: Value) -> bool {
        match (name, value) {
            ("position", Value::Float(position)) => {
                self.set_position(Dip(position as f32));
                true
            }
            _ => false,
        }
    }
}

/// Splits the area with the layout engine and places both panes and divider.
fn relayout<M>(ui: &Ui<M>, s: &Shared<M>) {
    let node = ui.bounds(s.id);
    if node.is_empty() {
        return;
    }
    // Children are parented to the split, so they are placed in its own
    // coordinates: start the panes at the origin.
    let bounds = Rect::from_size(node.size());
    let dpi = ui.dpi();
    let thickness = DIVIDER.to_px(dpi).value();
    let total = if s.horizontal {
        bounds.width()
    } else {
        bounds.height()
    };
    let available = (total - thickness).max(0);
    let (min_a, min_b) = clamp_mins(s, dpi, available);
    let position = clamp_position(s, dpi, available, min_a, min_b);

    let stack = if s.horizontal {
        Stack::horizontal()
    } else {
        Stack::vertical()
    };
    let rects = stack
        .push(StackSlot::FixedPx(Px(position)))
        .push(StackSlot::FixedPx(Px(thickness)))
        .push(StackSlot::Fill(1))
        .split(bounds, dpi);
    let (a, divider, b) = (rects[0], rects[1], rects[2]);
    s.current.set(position);
    s.divider.set(divider);

    let panes = s.panes.borrow();
    let mut moves: Vec<(WidgetId, Rect)> = Vec::with_capacity(panes.0.len() + panes.1.len() + 1);
    moves.push((s.divider_id, divider));
    for &child in &panes.0 {
        moves.push((child, a));
    }
    for &child in &panes.1 {
        moves.push((child, b));
    }
    drop(panes);
    ui.apply_moves(&moves);
    ui.invalidate(s.divider_id);
}

/// The minimum extents in pixels, clamped to what actually fits.
fn clamp_mins<M>(s: &Shared<M>, dpi: u32, available: i32) -> (i32, i32) {
    (
        s.min_a.get().to_px(dpi).value().clamp(0, available),
        s.min_b.get().to_px(dpi).value().clamp(0, available),
    )
}

/// The first pane's extent in pixels, honouring the design position and mins.
fn clamp_position<M>(s: &Shared<M>, dpi: u32, available: i32, min_a: i32, min_b: i32) -> i32 {
    let requested = match s.position.get() {
        Some(position) => position.to_px(dpi).value(),
        None => available / 2,
    }
    .clamp(0, available);
    if min_a + min_b <= available {
        requested.clamp(min_a, available - min_b)
    } else {
        available / 2
    }
}

/// Applies `px` as the first pane's extent, re-lays out and (optionally) raises
/// the move event.
fn apply_position<M>(ui: &Ui<M>, s: &Shared<M>, px: i32, emit: bool) {
    let dpi = ui.dpi();
    let bounds = Rect::from_size(ui.bounds(s.id).size());
    let thickness = DIVIDER.to_px(dpi).value();
    let total = if s.horizontal {
        bounds.width()
    } else {
        bounds.height()
    };
    let available = (total - thickness).max(0);
    let (min_a, min_b) = clamp_mins(s, dpi, available);
    let requested = px.clamp(0, available);
    let position = if min_a + min_b <= available {
        requested.clamp(min_a, available - min_b)
    } else {
        available / 2
    };
    let position = Px(position).to_dip(dpi);
    s.position.set(Some(position));
    relayout(ui, s);
    if emit {
        let mapped = s.on_moved.borrow().as_ref().and_then(|f| f(position));
        if let Some(msg) = mapped {
            ui.emit(msg);
        }
    }
}

/// Paints the divider: a filled band with a short grip.
fn paint_divider(canvas: &mut dyn Canvas, theme: Theme) {
    let bounds = canvas.bounds();
    backdrop(canvas, theme.background);
    canvas.fill_rect(bounds, theme.border);
    let (cx, cy) = (
        bounds.left + bounds.width() / 2,
        bounds.top + bounds.height() / 2,
    );
    canvas.draw_line(
        Point::new(cx, cy - 6),
        Point::new(cx, cy + 6),
        theme.text_secondary,
        1.0,
    );
}

/// The pointer's coordinate on the drag axis, in the split's own (stable)
/// coordinates: the event's divider-local point plus the divider's current
/// origin. The divider moves as the pane resizes, so its local origin shifts
/// under the cursor; converting on every event keeps the delta meaningful.
fn pointer_axis<M>(ui: &Ui<M>, s: &Shared<M>, x: i32, y: i32) -> i32 {
    let divider = ui.bounds(s.divider_id);
    if s.horizontal {
        divider.left + x
    } else {
        divider.top + y
    }
}

/// Handles divider input: drag, release and arrow keys.
fn divider_event<M>(s: &Shared<M>, ui: &Ui<M>, event: &Event) -> Option<M> {
    if ui.is_design_mode() && event.is_input() {
        return None;
    }
    let dpi = ui.dpi();
    match event {
        Event::MouseDown {
            x,
            y,
            button: MouseButton::Left,
            ..
        } => {
            s.drag.set(Some(pointer_axis(ui, s, *x, *y)));
            s.drag_start.set(s.current.get());
            ui.set_capture(s.divider_id);
        }
        Event::MouseMove { x, y, .. } => {
            if let Some(start) = s.drag.get() {
                let at = pointer_axis(ui, s, *x, *y);
                let target = s.drag_start.get() + (at - start);
                apply_position(ui, s, target, true);
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
        Event::KeyDown { key, .. } => {
            let step = ARROW_STEP.to_px(dpi).value();
            let delta = match *key {
                Key::LEFT | Key::UP => -step,
                Key::RIGHT | Key::DOWN => step,
                _ => return None,
            };
            apply_position(ui, s, s.current.get() + delta, true);
        }
        _ => {}
    }
    None
}
