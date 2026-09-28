#![forbid(unsafe_code)]

//! [`ComboBox`]: a drop-down list. A field shows the selected item; a popup
//! below it, hidden until clicked, lists the choices. Keys never raise it.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use super::control::{Control, HasText};
use crate::app::Ui;
use crate::backend::{NodeKind, NodeSpec, Result, WidgetId};
use crate::geometry::Rect;
use crate::property::{Properties, Property, Value};
use crate::units::Dip;

type SelectMapper<M> = RefCell<Option<Box<dyn Fn(usize) -> Option<M>>>>;

const TEXT_SIZE: Dip = Dip(12.0);
const ROW: Dip = Dip(24.0);
const PADDING: Dip = Dip(4.0);
const ARROW: Dip = Dip(8.0);

/// State the field and popup painters and event mappers share.
struct Shared<M: 'static> {
    items: Vec<String>,
    selected: Cell<usize>,
    open: Cell<bool>,
    hover: Cell<Option<usize>>,
    enabled: Cell<bool>,
    on_select: SelectMapper<M>,
}

/// A drop-down list of choices.
pub struct ComboBox<M: 'static> {
    field: Control<M>,
    popup: Control<M>,
    shared: Rc<Shared<M>>,
}

mod behavior;

use behavior::{field_event, paint_field, paint_popup, popup_event, popup_rect};

impl<M: 'static> ComboBox<M> {
    /// Creates a combo box with no bounds of its own, for a layout to place (see
    /// [`crate::arrange`]); its size comes from [`Placeable`](super::Placeable).
    pub fn auto(ui: &Ui<M>, items: &[&str]) -> Result<ComboBox<M>> {
        ComboBox::new(ui, Rect::default(), items)
    }

    /// Creates a combo box over `items` at `bounds`, the first item selected.
    pub fn new(ui: &Ui<M>, bounds: Rect, items: &[&str]) -> Result<ComboBox<M>> {
        let shared = Rc::new(Shared {
            items: items.iter().map(|i| i.to_string()).collect(),
            selected: Cell::new(0),
            open: Cell::new(false),
            hover: Cell::new(None),
            enabled: Cell::new(true),
            on_select: RefCell::new(None),
        });
        let first = shared.items.first().cloned().unwrap_or_default();
        let spec = NodeSpec::new(NodeKind::ComboBox, bounds).text(first);
        let field = Control::new(ui, &spec.tab_stop())?;
        let rect = popup_rect(bounds, ui.dpi(), shared.items.len());
        let popup = Control::new(ui, &NodeSpec::new(NodeKind::Custom, rect))?;
        ui.set_visible(popup.id(), false);
        let (field_id, popup_id) = (field.id(), popup.id());
        {
            let s = Rc::clone(&shared);
            let theme = ui.theme_handle();
            let flag = field.selected_handle();
            field.set_painter(Rc::new(move |c| {
                paint_field(&s, c, theme.get(), flag.get())
            }));
        }
        {
            let s = Rc::clone(&shared);
            let ui = ui.clone();
            field.on_events(move |e| field_event(&s, &ui, field_id, popup_id, e));
        }
        {
            let s = Rc::clone(&shared);
            let theme = ui.theme_handle();
            popup.set_painter(Rc::new(move |c| paint_popup(&s, c, theme.get())));
        }
        {
            let s = Rc::clone(&shared);
            let ui = ui.clone();
            popup.on_events(move |e| popup_event(&s, &ui, field_id, popup_id, e));
        }
        Ok(ComboBox {
            field,
            popup,
            shared,
        })
    }

    /// Maps a chosen row to the app's message: the closure receives the index
    /// and returns `Some(msg)` to raise it, or `None` to ignore it.
    pub fn on_select(self, mapper: impl Fn(usize) -> Option<M> + 'static) -> ComboBox<M> {
        *self.shared.on_select.borrow_mut() = Some(Box::new(mapper));
        self
    }

    /// The selected index.
    pub fn selected(&self) -> usize {
        self.shared.selected.get()
    }

    /// Selects `index` programmatically, updating the field text without
    /// raising the select event.
    pub fn select(&self, index: usize) {
        if let Some(item) = self.shared.items.get(index) {
            self.shared.selected.set(index);
            self.field.set_text(item);
            self.field.invalidate();
            self.popup.invalidate();
        }
    }

    /// The field's node identity.
    pub fn id(&self) -> WidgetId {
        self.field.id()
    }

    /// Enables or disables the combo box. A disabled box is dimmed and ignores
    /// input.
    pub fn set_enabled(&self, enabled: bool) {
        self.shared.enabled.set(enabled);
        for c in [&self.field, &self.popup] {
            c.set_enabled(enabled);
            c.invalidate();
        }
    }

    /// Marks the field selected, so its painter draws an outline (a form
    /// editor's selection).
    pub fn set_selected(&self, selected: bool) {
        self.field.set_selected(selected);
    }
}

impl<M: 'static> HasText for ComboBox<M> {
    /// The selected item's text.
    fn text(&self) -> String {
        match self.shared.items.get(self.shared.selected.get()) {
            Some(item) => item.clone(),
            None => String::new(),
        }
    }

    /// Selects the item equal to `text`; a no-op if no item matches.
    fn set_text(&self, text: &str) {
        if let Some(index) = self.shared.items.iter().position(|item| item == text) {
            self.select(index);
        }
    }
}

impl<M: 'static> Properties for ComboBox<M> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{App, Core, Runtime};
    use crate::backend::headless::HeadlessBackend;
    use crate::backend::{Backend, Event, PlatformSpec};
    use crate::message::{Key, Modifiers, MouseButton};

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

    fn visible(backend: &HeadlessBackend, id: WidgetId) -> bool {
        backend.node(id).expect("node").3
    }

    #[test]
    fn choosing_a_row_updates_and_raises_a_message() {
        let (backend, core, ui) = setup();
        let combo = ComboBox::new(&ui, Rect::new(0, 0, 120, 28), &["one", "two", "three"])
            .unwrap()
            .on_select(|i| Some(i as u32));
        let log = Rc::new(RefCell::new(Vec::new()));
        let rt = Runtime::primary(core, TestApp(Rc::clone(&log)));
        rt.deliver(combo.id(), &down(5, 5));
        let popup = combo.popup.id();
        assert!(visible(&backend, popup));
        // Events carry node-local coordinates, as the backend decodes them.
        let row = ROW.to_px(ui.dpi()).value().max(1);
        rt.deliver(popup, &down(5, row * 2 + row / 2));
        rt.deliver(WidgetId::NONE, &Event::Wake);
        assert_eq!(combo.selected(), 2);
        assert_eq!(combo.text(), "three");
        assert_eq!(*log.borrow(), vec![2]);
    }

    #[test]
    fn escape_closes_the_popup() {
        let (backend, core, ui) = setup();
        let combo = ComboBox::new(&ui, Rect::new(0, 0, 120, 28), &["one", "two"]).unwrap();
        let rt = Runtime::primary(core, TestApp(Rc::new(RefCell::new(Vec::new()))));
        let popup = combo.popup.id();
        assert!(!visible(&backend, popup), "the popup starts hidden");
        rt.deliver(combo.id(), &down(5, 5));
        assert!(visible(&backend, popup), "a click opens the popup");
        rt.deliver(
            combo.id(),
            &Event::KeyDown {
                key: Key::ESCAPE,
                modifiers: Modifiers::NONE,
                repeat: 1,
                system: false,
            },
        );
        assert!(!visible(&backend, popup), "Escape closes the popup");
    }
}
