#![forbid(unsafe_code)]

//! [`ListView`]: a fixed-row-height list of text rows with a single selection
//! and hover. It does not scroll (v1); only the rows that fit are drawn.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use super::control::Control;
use crate::app::Ui;
use crate::backend::{Event, NodeKind, NodeSpec, Result, TextStyle, WidgetId};
use crate::geometry::Rect;
use crate::message::{Key, MouseButton};
use crate::property::{Properties, Property, Value};
use crate::units::Dip;

/// Maps a selected or activated row to an optional app message.
type RowMapper<M> = Rc<RefCell<Option<Box<dyn Fn(usize) -> Option<M>>>>>;

/// The design height of one row.
const ROW: Dip = Dip(22.0);
/// The design size of a row's text.
const TEXT_SIZE: Dip = Dip(12.0);
/// The horizontal text inset inside a row.
const PADDING: Dip = Dip(6.0);

/// A list of rows with a single selection and hover highlight.
///
/// The selection maps to the app's `Msg` through [`ListView::on_select`] (a
/// left click) and [`ListView::on_activate`] (Return while a row is selected).
pub struct ListView<M: 'static> {
    control: Control<M>,
    items: Rc<RefCell<Vec<String>>>,
    selected: Rc<Cell<Option<usize>>>,
    hover: Rc<Cell<Option<usize>>>,
    enabled: Rc<Cell<bool>>,
    on_select: RowMapper<M>,
    on_activate: RowMapper<M>,
}

/// The row an event `y` (node-local) falls on; `None` above the first row or
/// below the last one.
fn row_at(dpi: u32, y: i32, count: usize) -> Option<usize> {
    if y < 0 {
        return None;
    }
    let row = ROW.to_px(dpi).value().max(1);
    let index = (y / row) as usize;
    (index < count).then_some(index)
}

impl<M: 'static> ListView<M> {
    /// Creates a list of `items` at `bounds`, with the first row selected (or
    /// no selection when empty).
    pub fn new(ui: &Ui<M>, bounds: Rect, items: &[&str]) -> Result<ListView<M>> {
        let control = Control::new(ui, &NodeSpec::new(NodeKind::ListView, bounds).tab_stop())?;
        let items: Vec<String> = items.iter().map(|item| item.to_string()).collect();
        let selected = if items.is_empty() { None } else { Some(0) };
        let items = Rc::new(RefCell::new(items));
        let selected = Rc::new(Cell::new(selected));
        let hover = Rc::new(Cell::new(None));
        let enabled = Rc::new(Cell::new(true));
        let on_select: RowMapper<M> = Rc::new(RefCell::new(None));
        let on_activate: RowMapper<M> = Rc::new(RefCell::new(None));

        {
            let items = Rc::clone(&items);
            let selected = Rc::clone(&selected);
            let hover = Rc::clone(&hover);
            let enabled = Rc::clone(&enabled);
            let theme = ui.theme_handle();
            let flag = control.selected_handle();
            control.set_painter(Rc::new(move |canvas| {
                let theme = theme.get();
                let bounds = canvas.bounds();
                let dpi = canvas.dpi();
                canvas.clear(theme.background);

                let row = ROW.to_px(dpi).value().max(1);
                let pad = PADDING.to_px(dpi).value();
                let enabled = enabled.get();
                for (index, item) in items.borrow().iter().enumerate() {
                    let top = bounds.top + row * index as i32;
                    let rect = Rect::new(bounds.left, top, bounds.right, top + row);
                    if selected.get() == Some(index) {
                        canvas.fill_rect(rect, theme.accent);
                    } else if hover.get() == Some(index) {
                        canvas.fill_rect(rect, theme.hover);
                    }
                    let color = if !enabled {
                        theme.text_disabled
                    } else if selected.get() == Some(index) {
                        theme.text_on_accent
                    } else {
                        theme.text
                    };
                    let text = Rect::new(rect.left + pad, rect.top, rect.right - pad, rect.bottom);
                    let style = TextStyle::new(color, TEXT_SIZE).middle();
                    canvas.draw_text(item, text, &style);
                    if index > 0 {
                        canvas.draw_line(
                            crate::geometry::Point::new(bounds.left, top),
                            crate::geometry::Point::new(bounds.right, top),
                            theme.border,
                            1.0,
                        );
                    }
                }
                if flag.get() {
                    canvas.stroke_rect(bounds, theme.accent, 2.0);
                }
            }));
        }

        {
            let items = Rc::clone(&items);
            let selected = Rc::clone(&selected);
            let hover = Rc::clone(&hover);
            let enabled = Rc::clone(&enabled);
            let on_select = Rc::clone(&on_select);
            let on_activate = Rc::clone(&on_activate);
            let ui = ui.clone();
            let id = control.id();
            control.on_events(move |event| {
                // In design mode the editor handles input, not the widget.
                if (ui.is_design_mode() && event.is_input()) || !enabled.get() {
                    return None;
                }
                match event {
                    Event::MouseDown {
                        y,
                        button: MouseButton::Left,
                        ..
                    } => {
                        let index = row_at(ui.dpi(), *y, items.borrow().len())?;
                        selected.set(Some(index));
                        ui.invalidate(id);
                        let mapper = on_select.borrow();
                        mapper.as_ref().and_then(|mapper| mapper(index))
                    }
                    Event::MouseMove { y, .. } => {
                        let index = row_at(ui.dpi(), *y, items.borrow().len());
                        if hover.get() != index {
                            hover.set(index);
                            ui.invalidate(id);
                        }
                        None
                    }
                    Event::MouseLeave => {
                        if hover.get().is_some() {
                            hover.set(None);
                            ui.invalidate(id);
                        }
                        None
                    }
                    Event::KeyDown {
                        key,
                        repeat,
                        system,
                        ..
                    } if *repeat <= 1 && !*system => {
                        let len = items.borrow().len();
                        if len == 0 {
                            return None;
                        }
                        let index = match *key {
                            Key::UP => selected.get().unwrap_or(0).saturating_sub(1),
                            Key::DOWN => (selected.get().unwrap_or(0) + 1).min(len - 1),
                            Key::HOME => 0,
                            Key::END => len - 1,
                            Key::RETURN => {
                                let index = selected.get()?;
                                let mapper = on_activate.borrow();
                                return mapper.as_ref().and_then(|mapper| mapper(index));
                            }
                            _ => return None,
                        };
                        selected.set(Some(index));
                        ui.invalidate(id);
                        None
                    }
                    _ => None,
                }
            });
        }

        Ok(ListView {
            control,
            items,
            selected,
            hover,
            enabled,
            on_select,
            on_activate,
        })
    }

    /// Maps selecting a row to the app's message: the closure receives the
    /// index and returns `Some(msg)` to raise it, or `None` to ignore it.
    pub fn on_select(self, mapper: impl Fn(usize) -> Option<M> + 'static) -> ListView<M> {
        *self.on_select.borrow_mut() = Some(Box::new(mapper));
        self
    }

    /// Maps activating the selected row (Return) to the app's message.
    pub fn on_activate(self, mapper: impl Fn(usize) -> Option<M> + 'static) -> ListView<M> {
        *self.on_activate.borrow_mut() = Some(Box::new(mapper));
        self
    }

    /// The selected row, if any.
    pub fn selected(&self) -> Option<usize> {
        self.selected.get()
    }

    /// Selects `index` without raising an event; an out-of-range index clears
    /// the selection.
    pub fn select(&self, index: Option<usize>) {
        let len = self.items.borrow().len();
        self.selected.set(index.filter(|index| *index < len));
        self.control.invalidate();
    }

    /// Replaces the rows. A selection past the new end is moved to the last row.
    pub fn set_items(&self, items: &[&str]) {
        let mut stored = self.items.borrow_mut();
        *stored = items.iter().map(|item| item.to_string()).collect();
        let len = stored.len();
        drop(stored);
        if self.selected.get().is_some_and(|index| index >= len) {
            self.selected
                .set(if len == 0 { None } else { Some(len - 1) });
        }
        self.hover.set(None);
        self.control.invalidate();
    }

    /// The number of rows.
    pub fn len(&self) -> usize {
        self.items.borrow().len()
    }

    /// Whether the list has no rows.
    pub fn is_empty(&self) -> bool {
        self.items.borrow().is_empty()
    }

    /// The list's node identity.
    pub fn id(&self) -> WidgetId {
        self.control.id()
    }

    /// Enables or disables the list. A disabled list is dimmed and ignores
    /// input.
    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.set(enabled);
        self.control.set_enabled(enabled);
        self.control.invalidate();
    }

    /// Marks the list selected, so its painter draws an outline (a form
    /// editor's selection).
    pub fn set_selected(&self, selected: bool) {
        self.control.set_selected(selected);
    }
}

impl<M: 'static> Properties for ListView<M> {
    fn properties(&self) -> Vec<Property> {
        vec![Property {
            name: "selected",
            value: Value::Integer(self.selected().map_or(-1, |index| index as i64)),
        }]
    }

    fn set_property(&self, name: &str, value: Value) -> bool {
        match (name, value) {
            ("selected", Value::Integer(index)) => {
                self.select(if index < 0 {
                    None
                } else {
                    Some(index as usize)
                });
                true
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use super::*;
    use crate::app::{App, Core, Runtime};
    use crate::backend::headless::HeadlessBackend;
    use crate::backend::{Backend, PlatformSpec};
    use crate::message::Modifiers;

    struct TestApp(Rc<RefCell<Vec<u32>>>);

    impl App for TestApp {
        type Msg = u32;

        fn update(&mut self, msg: u32, _ui: &mut Ui<u32>) {
            self.0.borrow_mut().push(msg);
        }
    }

    fn setup() -> (Rc<HeadlessBackend>, Rc<Core<u32>>, Ui<u32>) {
        let backend = Rc::new(HeadlessBackend::new());
        let window = backend.open_window(&PlatformSpec::new("test")).unwrap();
        let core = Core::new(backend.clone(), window);
        let ui = Ui::new(Rc::clone(&core));
        (backend, core, ui)
    }

    fn down(x: i32, y: i32) -> Event {
        Event::MouseDown {
            x,
            y,
            button: MouseButton::Left,
            modifiers: Modifiers::NONE,
        }
    }

    fn key(key: Key) -> Event {
        Event::KeyDown {
            key,
            modifiers: Modifiers::NONE,
            repeat: 1,
            system: false,
        }
    }

    #[test]
    fn clicking_a_row_selects_and_raises_a_message() {
        let (_backend, core, ui) = setup();
        let list = ListView::new(&ui, Rect::new(0, 0, 120, 88), &["one", "two", "three"])
            .unwrap()
            .on_select(|index| Some(index as u32));
        let log = Rc::new(RefCell::new(Vec::new()));
        let runtime = Runtime::primary(core, TestApp(Rc::clone(&log)));

        let row = ROW.to_px(ui.dpi()).value().max(1);
        runtime.deliver(list.id(), &down(5, row + row / 2));
        runtime.deliver(WidgetId::NONE, &Event::Wake);

        assert_eq!(list.selected(), Some(1));
        assert_eq!(*log.borrow(), vec![1]);
    }

    #[test]
    fn a_hit_test_uses_node_local_coordinates() {
        let (_backend, core, ui) = setup();
        // A list away from the origin: the event's `y` is relative to the
        // node's client area, not the window.
        let list = ListView::new(&ui, Rect::new(40, 200, 160, 288), &["one", "two", "three"])
            .unwrap()
            .on_select(|index| Some(index as u32));
        let log = Rc::new(RefCell::new(Vec::new()));
        let runtime = Runtime::primary(core, TestApp(Rc::clone(&log)));

        let row = ROW.to_px(ui.dpi()).value().max(1);
        runtime.deliver(list.id(), &down(5, row + row / 2));
        runtime.deliver(WidgetId::NONE, &Event::Wake);

        assert_eq!(list.selected(), Some(1));
        assert_eq!(*log.borrow(), vec![1]);
    }

    #[test]
    fn down_then_return_activates_a_row() {
        let (_backend, core, ui) = setup();
        let list = ListView::new(&ui, Rect::new(0, 0, 120, 88), &["one", "two", "three"])
            .unwrap()
            .on_activate(|index| Some(index as u32));
        let log = Rc::new(RefCell::new(Vec::new()));
        let runtime = Runtime::primary(core, TestApp(Rc::clone(&log)));

        runtime.deliver(list.id(), &key(Key::DOWN));
        runtime.deliver(list.id(), &key(Key::RETURN));
        runtime.deliver(WidgetId::NONE, &Event::Wake);

        assert_eq!(list.selected(), Some(1));
        assert_eq!(*log.borrow(), vec![1]);
    }

    #[test]
    fn select_is_programmatic_and_raises_nothing() {
        let (_backend, core, ui) = setup();
        let list = ListView::new(&ui, Rect::new(0, 0, 120, 88), &["one", "two", "three"])
            .unwrap()
            .on_select(|index| Some(index as u32));
        let log = Rc::new(RefCell::new(Vec::new()));
        let runtime = Runtime::primary(core, TestApp(Rc::clone(&log)));

        list.select(Some(2));
        runtime.deliver(WidgetId::NONE, &Event::Wake);

        assert_eq!(list.selected(), Some(2));
        assert!(log.borrow().is_empty());
    }
}
