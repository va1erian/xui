#![forbid(unsafe_code)]

//! [`Toolbar`]: a horizontal strip of clickable label items.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use super::control::Control;
use crate::app::Ui;
use crate::backend::{Event, NodeKind, NodeSpec, Result, TextStyle, WidgetId};
use crate::geometry::{Point, Rect};
use crate::message::{Key, MouseButton};
use crate::property::{Properties, Property, Value};
use crate::units::Dip;

/// Maps the index of a clicked item to an optional app message.
type ClickMapper<M> = Rc<RefCell<Option<Box<dyn Fn(usize) -> Option<M>>>>>;

/// The corner radius of an item's highlight.
const RADIUS: f32 = 4.0;
/// The design size of an item's label.
const TEXT_SIZE: Dip = Dip(12.0);

/// The item an event `x` (node-local) falls on; `None` left of the first item
/// or right of the last one. `bounds` supplies the strip's total width.
fn item_at(bounds: Rect, x: i32, count: usize) -> Option<usize> {
    if count == 0 || x < 0 {
        return None;
    }
    let width = bounds.width() / count as i32;
    if width <= 0 {
        return None;
    }
    let index = (x / width) as usize;
    (index < count).then_some(index)
}

/// A horizontal strip of equally-spaced label items.
///
/// Clicking an item maps its index to the app's `Msg` through
/// [`Toolbar::on_click`]. The left and right arrow keys move a focused item
/// (shown with the hover highlight) and Return activates it; a disabled
/// toolbar ignores input and dims its labels.
pub struct Toolbar<M: 'static> {
    control: Control<M>,
    items: Rc<Vec<String>>,
    hover: Rc<Cell<Option<usize>>>,
    enabled: Rc<Cell<bool>>,
    activated: Rc<Cell<i64>>,
    on_click: ClickMapper<M>,
}

impl<M: 'static> Toolbar<M> {
    /// Creates a toolbar of `items` along `bounds`.
    pub fn new(ui: &Ui<M>, bounds: Rect, items: &[&str]) -> Result<Toolbar<M>> {
        let control = Control::new(ui, &NodeSpec::new(NodeKind::Toolbar, bounds).tab_stop())?;
        let items: Rc<Vec<String>> = Rc::new(items.iter().map(|item| item.to_string()).collect());
        let hover = Rc::new(Cell::new(None));
        let pressed = Rc::new(Cell::new(None));
        let enabled = Rc::new(Cell::new(true));
        let activated = Rc::new(Cell::new(-1i64));
        let on_click: ClickMapper<M> = Rc::new(RefCell::new(None));

        {
            let items = Rc::clone(&items);
            let hover = Rc::clone(&hover);
            let pressed = Rc::clone(&pressed);
            let enabled = Rc::clone(&enabled);
            let theme = ui.theme_handle();
            let flag = control.selected_handle();
            control.set_painter(Rc::new(move |canvas| {
                let theme = theme.get();
                let bounds = canvas.bounds();
                canvas.clear(theme.background);

                let count = items.len();
                let width = if count == 0 {
                    0
                } else {
                    (bounds.width() / count as i32).max(1)
                };
                let enabled = enabled.get();
                for (index, item) in items.iter().enumerate() {
                    let left = bounds.left + width * index as i32;
                    let right = if index + 1 == count {
                        bounds.right
                    } else {
                        left + width
                    };
                    let rect = Rect::new(left, bounds.top, right, bounds.bottom);
                    if pressed.get() == Some(index) {
                        canvas.fill_rounded_rect(rect, RADIUS, theme.pressed);
                    } else if hover.get() == Some(index) {
                        canvas.fill_rounded_rect(rect, RADIUS, theme.hover);
                    }
                    if index > 0 {
                        canvas.draw_line(
                            Point::new(left, bounds.top),
                            Point::new(left, bounds.bottom),
                            theme.border,
                            1.0,
                        );
                    }
                    let color = if enabled {
                        theme.text
                    } else {
                        theme.text_disabled
                    };
                    let style = TextStyle::new(color, TEXT_SIZE).centered().middle();
                    canvas.draw_text(item, rect, &style);
                }
                if flag.get() {
                    canvas.stroke_rect(bounds, theme.accent, 2.0);
                }
            }));
        }

        {
            let items = Rc::clone(&items);
            let hover = Rc::clone(&hover);
            let pressed = Rc::clone(&pressed);
            let enabled = Rc::clone(&enabled);
            let activated = Rc::clone(&activated);
            let on_click = Rc::clone(&on_click);
            let ui = ui.clone();
            let id = control.id();
            control.on_events(move |event| {
                // In design mode the editor handles input, not the widget.
                if ui.is_design_mode() && event.is_input() {
                    return None;
                }
                if !enabled.get() {
                    return None;
                }
                let bounds = ui.bounds(id);
                match event {
                    Event::MouseMove { x, .. } => {
                        let index = item_at(bounds, *x, items.len());
                        if hover.get() != index {
                            hover.set(index);
                            ui.invalidate(id);
                        }
                        None
                    }
                    Event::MouseLeave | Event::CaptureChanged => {
                        if hover.get().is_some() || pressed.get().is_some() {
                            hover.set(None);
                            pressed.set(None);
                            ui.invalidate(id);
                        }
                        None
                    }
                    Event::MouseDown {
                        x,
                        button: MouseButton::Left,
                        ..
                    } => {
                        pressed.set(item_at(bounds, *x, items.len()));
                        ui.invalidate(id);
                        None
                    }
                    Event::MouseUp {
                        x,
                        button: MouseButton::Left,
                        ..
                    } => {
                        let index = item_at(bounds, *x, items.len());
                        let was_pressed = pressed.get();
                        pressed.set(None);
                        ui.invalidate(id);
                        if index.is_none() || index != was_pressed {
                            return None;
                        }
                        let index = index?;
                        activated.set(index as i64);
                        let mapper = on_click.borrow();
                        mapper.as_ref().and_then(|mapper| mapper(index))
                    }
                    Event::KeyDown {
                        key,
                        repeat,
                        system,
                        ..
                    } if *repeat <= 1 && !*system => {
                        let count = items.len();
                        if count == 0 {
                            return None;
                        }
                        match *key {
                            Key::LEFT => {
                                let index = hover.get().unwrap_or(0).saturating_sub(1);
                                hover.set(Some(index));
                                ui.invalidate(id);
                                None
                            }
                            Key::RIGHT => {
                                let index = (hover.get().unwrap_or(0) + 1).min(count - 1);
                                hover.set(Some(index));
                                ui.invalidate(id);
                                None
                            }
                            Key::RETURN => {
                                let index = hover.get()?;
                                activated.set(index as i64);
                                ui.invalidate(id);
                                let mapper = on_click.borrow();
                                mapper.as_ref().and_then(|mapper| mapper(index))
                            }
                            _ => None,
                        }
                    }
                    _ => None,
                }
            });
        }

        Ok(Toolbar {
            control,
            items,
            hover,
            enabled,
            activated,
            on_click,
        })
    }

    /// Maps a click to the app's message: the closure receives the item index
    /// and returns `Some(msg)` to raise it, or `None` to ignore the click.
    pub fn on_click(self, mapper: impl Fn(usize) -> Option<M> + 'static) -> Toolbar<M> {
        *self.on_click.borrow_mut() = Some(Box::new(mapper));
        self
    }

    /// The number of items.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Whether the toolbar has no items.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// The toolbar's node identity.
    pub fn id(&self) -> WidgetId {
        self.control.id()
    }

    /// Enables or disables the toolbar. A disabled toolbar is dimmed and
    /// ignores input.
    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.set(enabled);
        self.control.set_enabled(enabled);
        self.control.invalidate();
    }

    /// Whether the toolbar is enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled.get()
    }

    /// Marks the toolbar selected, so its painter draws an outline (a form
    /// editor's selection).
    pub fn set_selected(&self, selected: bool) {
        self.control.set_selected(selected);
    }
}

impl<M: 'static> Properties for Toolbar<M> {
    fn properties(&self) -> Vec<Property> {
        vec![Property {
            name: "selected",
            value: Value::Integer(self.activated.get()),
        }]
    }

    fn set_property(&self, name: &str, value: Value) -> bool {
        match (name, value) {
            ("selected", Value::Integer(index)) => {
                let in_range = index >= 0 && (index as usize) < self.items.len();
                let index = if in_range { index } else { -1 };
                self.activated.set(index);
                self.hover.set((index >= 0).then_some(index as usize));
                self.control.invalidate();
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

    fn down(x: i32) -> Event {
        Event::MouseDown {
            x,
            y: 5,
            button: MouseButton::Left,
            modifiers: Modifiers::NONE,
        }
    }

    fn up(x: i32) -> Event {
        Event::MouseUp {
            x,
            y: 5,
            button: MouseButton::Left,
            modifiers: Modifiers::NONE,
        }
    }

    #[test]
    fn clicking_an_item_maps_to_the_apps_message() {
        let (_backend, core, ui) = setup();
        let toolbar = Toolbar::new(&ui, Rect::new(0, 0, 90, 28), &["one", "two", "three"])
            .unwrap()
            .on_click(|index| Some(index as u32));
        let log = Rc::new(RefCell::new(Vec::new()));
        let runtime = Runtime::primary(core, TestApp(Rc::clone(&log)));

        runtime.deliver(toolbar.id(), &down(45));
        runtime.deliver(toolbar.id(), &up(45));
        runtime.deliver(WidgetId::NONE, &Event::Wake);

        assert_eq!(*log.borrow(), vec![1]);
        assert_eq!(toolbar.property("selected"), Some(Value::Integer(1)));
    }

    #[test]
    fn a_click_past_the_last_item_raises_nothing() {
        let (_backend, core, ui) = setup();
        let toolbar = Toolbar::new(&ui, Rect::new(0, 0, 90, 28), &["one", "two", "three"])
            .unwrap()
            .on_click(|index| Some(index as u32));
        let log = Rc::new(RefCell::new(Vec::new()));
        let runtime = Runtime::primary(core, TestApp(Rc::clone(&log)));

        runtime.deliver(toolbar.id(), &down(95));
        runtime.deliver(toolbar.id(), &up(95));
        runtime.deliver(WidgetId::NONE, &Event::Wake);

        assert!(log.borrow().is_empty(), "no item is under x=95");
    }
}
