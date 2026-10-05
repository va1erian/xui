#![forbid(unsafe_code)]

//! [`ColorPicker`]: a portable grid of colour swatches.
//!
//! The picker paints every colour as a rounded swatch and selects one when it
//! is clicked or activated from the keyboard. The chosen [`Color`] reaches the
//! app through [`ColorPicker::on_select`]; a settings page uses it for an
//! accent-colour palette.
//!
//! The whole grid is one painted node: it draws only the swatch rectangles, so
//! adding a colour does not add a control.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use super::Placeable;
use super::control::Control;
use crate::Color;
use crate::app::Ui;
use crate::backend::{Canvas, Event, NodeKind, NodeSpec, Result};
use crate::geometry::{Point, Rect, Size};
use crate::layout::Constraints;
use crate::message::{Key, MouseButton};
use crate::theme::look::{self, backdrop};
use crate::units::Dip;

/// Maps a chosen colour to an optional app message.
type SelectMapper<M> = Rc<RefCell<Option<Box<dyn Fn(Color) -> Option<M>>>>>;

/// Padding between a swatch and its cell edge.
const GAP: Dip = Dip(4.0);
/// The swatch corner radius, in pixels.
const RADIUS: f32 = 6.0;
/// The default number of columns when [`ColorPicker::columns`] is not called.
const DEFAULT_COLUMNS: usize = 8;

/// A grid of colour swatches that selects one colour.
pub struct ColorPicker<M: 'static> {
    control: Control<M>,
    colors: Rc<Vec<Color>>,
    columns: Rc<Cell<usize>>,
    selected: Rc<Cell<Option<usize>>>,
    on_select: SelectMapper<M>,
}

impl<M: 'static> ColorPicker<M> {
    /// Creates a picker over `colors`.
    pub fn new(ui: &Ui<M>, bounds: Rect, colors: &[Color]) -> Result<ColorPicker<M>> {
        let control = Control::new(ui, &NodeSpec::new(NodeKind::Custom, bounds))?;
        let colors = Rc::new(colors.to_vec());
        let columns = Rc::new(Cell::new(DEFAULT_COLUMNS));
        let selected = Rc::new(Cell::new(None));
        let hover = Rc::new(Cell::new(None));
        let on_select: SelectMapper<M> = Rc::new(RefCell::new(None));

        {
            let colors = Rc::clone(&colors);
            let selected = Rc::clone(&selected);
            let hover = Rc::clone(&hover);
            let columns = Rc::clone(&columns);
            let theme = ui.theme_handle();
            let selected_flag = control.selected_handle();
            control.set_painter(Rc::new(move |canvas| {
                let theme = theme.get();
                let bounds = canvas.bounds();
                backdrop(canvas, theme.background);
                let count = colors.len();
                for index in 0..count {
                    let cell = cell_rect(bounds, columns.get(), count, index);
                    let gap = GAP.to_px(canvas.dpi()).value();
                    let swatch = cell.shrink(gap);
                    if swatch.is_empty() {
                        continue;
                    }
                    let is_selected = selected.get() == Some(index);
                    if look::decorated(&theme) {
                        paint_glossy(
                            canvas,
                            &theme,
                            swatch,
                            colors[index],
                            is_selected,
                            hover.get() == Some(index),
                        );
                        continue;
                    }
                    let radius = RADIUS;
                    canvas.fill_rounded_rect(swatch, radius, colors[index]);
                    let ring = if selected.get() == Some(index) {
                        theme.accent
                    } else if hover.get() == Some(index) {
                        theme.border_focused
                    } else {
                        theme.border
                    };
                    let width = if selected.get() == Some(index) {
                        2.0
                    } else {
                        1.0
                    };
                    canvas.stroke_rounded_rect(swatch, radius, ring, width);
                    if selected.get() == Some(index) {
                        draw_check(canvas, swatch, ink_on(colors[index]));
                    }
                }
                if selected_flag.get() {
                    canvas.stroke_rect(bounds, theme.accent, 2.0);
                }
            }));
        }

        {
            let colors = Rc::clone(&colors);
            let selected = Rc::clone(&selected);
            let hover = Rc::clone(&hover);
            let columns = Rc::clone(&columns);
            let on_select = Rc::clone(&on_select);
            let ui_for = ui.clone();
            let id = control.id();
            control.on_events(move |event| {
                if ui_for.is_design_mode() && event.is_input() {
                    return None;
                }
                match event {
                    Event::MouseMove { x, y, .. } => {
                        hover.set(index_at(
                            local_bounds(&ui_for, id),
                            colors.len(),
                            columns.get(),
                            *x,
                            *y,
                        ));
                        ui_for.invalidate(id);
                        None
                    }
                    Event::MouseLeave | Event::CaptureChanged => {
                        hover.set(None);
                        ui_for.invalidate(id);
                        None
                    }
                    Event::MouseDown {
                        x,
                        y,
                        button: MouseButton::Left,
                        ..
                    } => {
                        let index = index_at(
                            local_bounds(&ui_for, id),
                            colors.len(),
                            columns.get(),
                            *x,
                            *y,
                        )?;
                        ui_for.focus(id);
                        raise(&ui_for, id, &colors, &selected, &hover, &on_select, index)
                    }
                    Event::KeyDown {
                        key,
                        repeat,
                        system,
                        ..
                    } if *repeat <= 1 && !*system => {
                        let index = moved(&colors, &selected, columns.get(), *key)?;
                        raise(&ui_for, id, &colors, &selected, &hover, &on_select, index)
                    }
                    _ => None,
                }
            });
        }

        Ok(ColorPicker {
            control,
            colors,
            columns,
            selected,
            on_select,
        })
    }

    /// Sets the number of swatches per row.
    pub fn columns(self, columns: usize) -> ColorPicker<M> {
        self.columns.set(columns.max(1));
        self.control.invalidate();
        self
    }

    /// Sets the initially selected colour.
    pub fn selected(self, color: Color) -> ColorPicker<M> {
        let index = self.colors.iter().position(|candidate| *candidate == color);
        self.selected.set(index);
        self.control.invalidate();
        self
    }

    /// Maps a chosen colour to the app's message: the closure returns
    /// `Some(msg)` to raise it, or `None` to ignore the pick.
    pub fn on_select(self, mapper: impl Fn(Color) -> Option<M> + 'static) -> ColorPicker<M> {
        *self.on_select.borrow_mut() = Some(Box::new(mapper));
        self
    }

    /// The currently selected colour, if any.
    pub fn color(&self) -> Option<Color> {
        self.selected.get().map(|index| self.colors[index])
    }

    /// Selects `color` from code without raising `on_select`.
    pub fn select(&self, color: Color) {
        self.selected
            .set(self.colors.iter().position(|candidate| *candidate == color));
        self.control.invalidate();
    }

    /// The picker's node identity.
    pub fn id(&self) -> crate::backend::WidgetId {
        self.control.id()
    }

    /// Marks the picker selected, so its painter draws an outline (a form
    /// editor's selection).
    pub fn set_selected(&self, selected: bool) {
        self.control.set_selected(selected);
    }
}

/// The natural side of one swatch cell.
const CELL: Dip = Dip(32.0);

impl<M: 'static> Placeable<M> for ColorPicker<M> {
    fn id(&self) -> crate::backend::WidgetId {
        ColorPicker::id(self)
    }

    /// One square cell per colour, in the picker's columns.
    fn measure(&self, _ui: &Ui<M>, constraints: Constraints) -> Size {
        let cell = CELL.to_px(constraints.dpi).value();
        let columns = self.columns.get().max(1);
        let rows = self.colors.len().div_ceil(columns);
        let shown = columns.min(self.colors.len());
        Size::new(cell * shown as i32, cell * rows as i32)
    }
}

/// Selects `index`, repaints and raises the mapped message.
fn raise<M: 'static>(
    ui: &Ui<M>,
    id: crate::backend::WidgetId,
    colors: &Rc<Vec<Color>>,
    selected: &Rc<Cell<Option<usize>>>,
    hover: &Rc<Cell<Option<usize>>>,
    on_select: &SelectMapper<M>,
    index: usize,
) -> Option<M> {
    selected.set(Some(index));
    hover.set(Some(index));
    ui.invalidate(id);
    let mapper = on_select.borrow();
    mapper.as_ref().and_then(|mapper| mapper(colors[index]))
}

/// The index an arrow/Home/End key moves the selection to.
fn moved(
    colors: &Rc<Vec<Color>>,
    selected: &Rc<Cell<Option<usize>>>,
    columns: usize,
    key: Key,
) -> Option<usize> {
    let count = colors.len();
    if count == 0 {
        return None;
    }
    let current = selected.get().unwrap_or(0);
    let next = match key {
        Key::LEFT => current.saturating_sub(1),
        Key::RIGHT => (current + 1).min(count - 1),
        Key::UP => current.saturating_sub(columns.max(1)),
        Key::DOWN => (current + columns.max(1)).min(count - 1),
        Key::HOME => 0,
        Key::END => count - 1,
        Key::SPACE | Key::RETURN => current,
        _ => return None,
    };
    Some(next)
}

/// The grid's extent in the node's own coordinates: pointer events arrive
/// relative to the node, while `Ui::bounds` is relative to its parent, so only
/// the size of the latter is meaningful here. Using its origin too made a
/// picker that is not at its container's top-left ignore every click.
fn local_bounds<M: 'static>(ui: &Ui<M>, id: crate::backend::WidgetId) -> Rect {
    let bounds = ui.bounds(id);
    Rect::new(0, 0, bounds.width(), bounds.height())
}

/// The swatch under `(x, y)` (node-local), if the point is inside the grid.
fn index_at(bounds: Rect, count: usize, columns: usize, x: i32, y: i32) -> Option<usize> {
    if count == 0 || !bounds.contains(Point::new(x, y)) {
        return None;
    }
    let columns = columns.max(1);
    let rows = count.div_ceil(columns).max(1);
    let cell_w = (bounds.width() / columns as i32).max(1);
    let cell_h = (bounds.height() / rows as i32).max(1);
    let column = ((x - bounds.left) / cell_w).clamp(0, columns as i32 - 1) as usize;
    let row = ((y - bounds.top) / cell_h).clamp(0, rows as i32 - 1) as usize;
    let index = row * columns + column;
    (index < count).then_some(index)
}

/// The rectangle of the swatch at `index`, in canvas coordinates.
fn cell_rect(bounds: Rect, columns: usize, count: usize, index: usize) -> Rect {
    let columns = columns.max(1);
    let rows = count.div_ceil(columns).max(1);
    let cell_w = (bounds.width() / columns as i32).max(1);
    let cell_h = (bounds.height() / rows as i32).max(1);
    let row = index / columns;
    let column = index % columns;
    let left = bounds.left + column as i32 * cell_w;
    let top = bounds.top + row as i32 * cell_h;
    Rect::new(left, top, left + cell_w, top + cell_h)
}

/// Draws a check mark inside a swatch.
/// A decorated theme's swatch: a glossy disc (or rounded square where the
/// cell is not square), ringed apart from it in the accent with a glow when
/// selected, and in the focus colour when hovered.
fn paint_glossy(
    canvas: &mut dyn Canvas,
    theme: &crate::theme::Theme,
    cell: Rect,
    color: Color,
    selected: bool,
    hovered: bool,
) {
    // Room for the ring outside the swatch.
    let swatch = cell.shrink(3);
    if swatch.is_empty() {
        return;
    }
    let square = (swatch.width() - swatch.height()).abs() <= 4;
    let radius = if square {
        swatch.width().min(swatch.height()) as f32 / 2.0
    } else {
        RADIUS
    };
    let ring = cell;
    let ring_radius = radius + 3.0;
    if selected {
        let center = Point::new(cell.left + cell.width() / 2, cell.top + cell.height() / 2);
        look::glow(canvas, center, ring_radius, theme);
    }
    look::face(canvas, swatch, radius, color, theme);
    if selected {
        canvas.stroke_rounded_rect(ring, ring_radius, theme.accent, 2.0);
        draw_check(canvas, swatch, ink_on(color));
    } else if hovered {
        canvas.stroke_rounded_rect(ring, ring_radius, theme.border_focused, 1.0);
    }
}

fn draw_check(canvas: &mut dyn Canvas, swatch: Rect, ink: Color) {
    // Draw the tick in a centred square of the swatch's shorter side, so a wide
    // swatch shows the check's usual shape instead of a stretched one.
    let side = swatch.width().min(swatch.height());
    let left = swatch.left + (swatch.width() - side) / 2;
    let top = swatch.top + (swatch.height() - side) / 2;
    let first = Point::new(left + side * 28 / 100, top + side * 52 / 100);
    let corner = Point::new(left + side * 45 / 100, top + side * 70 / 100);
    let last = Point::new(left + side * 74 / 100, top + side * 30 / 100);
    canvas.draw_line(first, corner, ink, 2.0);
    canvas.draw_line(corner, last, ink, 2.0);
}

/// A legible ink for a swatch: white or black, whichever contrasts more.
fn ink_on(color: Color) -> Color {
    let white = Color::rgb(255, 255, 255);
    let black = Color::rgb(0, 0, 0);
    if color.contrast_ratio(white) >= color.contrast_ratio(black) {
        white
    } else {
        black
    }
}

#[cfg(test)]
mod tests;
