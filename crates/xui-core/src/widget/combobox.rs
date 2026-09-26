#![forbid(unsafe_code)]

//! [`ComboBox`]: a drop-down list. A field shows the selected item; a popup
//! below it, hidden until clicked, lists the choices. Keys never raise it.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use super::control::{Control, HasText};
use crate::app::Ui;
use crate::backend::{Canvas, Event, NodeKind, NodeSpec, Result, TextStyle, WidgetId};
use crate::geometry::{Point, Rect};
use crate::message::{Key, MouseButton};
use crate::property::{Properties, Property, Value};
use crate::theme::Theme;
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

fn pick<T>(cond: bool, yes: T, no: T) -> T {
    if cond { yes } else { no }
}

/// The popup's rectangle: directly below `field`, one row per item.
fn popup_rect(field: Rect, dpi: u32, count: usize) -> Rect {
    let row = ROW.to_px(dpi).value().max(1);
    let bottom = field.bottom + row * count as i32;
    Rect::new(field.left, field.bottom, field.right, bottom)
}

/// Shows or hides the popup to match `want`, moving it under the field first.
fn set_open<M: 'static>(
    ui: &Ui<M>,
    (field, popup): (WidgetId, WidgetId),
    s: &Shared<M>,
    want: bool,
) {
    if want == s.open.replace(want) {
        return;
    }
    if want {
        ui.apply_moves(&[(popup, popup_rect(ui.bounds(field), ui.dpi(), s.items.len()))]);
    }
    ui.set_visible(popup, want);
    ui.invalidate(field);
    ui.invalidate(popup);
}

/// The row an event `y` falls on, relative to the popup's bounds.
fn row_at<M: 'static>(ui: &Ui<M>, popup: WidgetId, y: i32, count: usize) -> Option<usize> {
    let rel = y - ui.bounds(popup).top;
    (rel >= 0)
        .then(|| (rel / ROW.to_px(ui.dpi()).value().max(1)) as usize)
        .filter(|index| *index < count)
}

/// Paints the field: input background, border, selected text and a chevron.
fn paint_field<M: 'static>(s: &Shared<M>, canvas: &mut dyn Canvas, theme: Theme, flag: bool) {
    let b = canvas.bounds();
    let dpi = canvas.dpi();
    let (pad, arrow) = (PADDING.to_px(dpi).value(), ARROW.to_px(dpi).value());
    canvas.clear(theme.input_background);
    let color = pick(s.enabled.get(), theme.text, theme.text_disabled);
    let border = pick(s.open.get(), theme.border_focused, theme.border);
    canvas.stroke_rect(b, border, 1.0);
    let i = b.shrink(pad);
    let text = Rect::new(i.left, i.top, (i.right - arrow).max(i.left), i.bottom);
    if let Some(item) = s.items.get(s.selected.get()) {
        canvas.draw_text(item, text, &TextStyle::new(color, TEXT_SIZE).middle());
    }
    let (cx, cy) = (b.right - pad - arrow / 2, b.top + b.height() / 2);
    let half = (arrow / 2).max(2);
    let p1 = Point::new(cx - half, cy - half / 2);
    let p2 = Point::new(cx, cy + half / 2);
    let p3 = Point::new(cx + half, cy - half / 2);
    canvas.draw_line(p1, p2, color, 1.5);
    canvas.draw_line(p2, p3, color, 1.5);
    if flag {
        canvas.stroke_rect(b, theme.accent, 2.0);
    }
}

/// Paints every popup row, highlighting the selected and hovered ones.
fn paint_popup<M: 'static>(s: &Shared<M>, canvas: &mut dyn Canvas, theme: Theme) {
    let b = canvas.bounds();
    let row = ROW.to_px(canvas.dpi()).value().max(1);
    let pad = PADDING.to_px(canvas.dpi()).value();
    canvas.clear(theme.surface);
    canvas.stroke_rect(b, theme.border, 1.0);
    for (index, item) in s.items.iter().enumerate() {
        let top = b.top + row * index as i32;
        let rect = Rect::new(b.left, top, b.right, top + row);
        let hot = index == s.selected.get() || s.hover.get() == Some(index);
        if hot {
            canvas.fill_rect(rect, theme.selection);
        }
        let hot_color = pick(hot, theme.text_on_accent, theme.text);
        let color = pick(s.enabled.get(), hot_color, theme.text_disabled);
        let style = TextStyle::new(color, TEXT_SIZE).middle();
        canvas.draw_text(item, rect.shrink(pad), &style);
    }
}

/// Handles field input: click and Space/Return toggle, Escape closes, arrows
/// move the selection.
fn field_event<M: 'static>(
    s: &Shared<M>,
    ui: &Ui<M>,
    field: WidgetId,
    popup: WidgetId,
    event: &Event,
) -> Option<M> {
    if (ui.is_design_mode() && event.is_input()) || !s.enabled.get() {
        return None;
    }
    let len = s.items.len();
    let toggle = !s.open.get();
    match event {
        Event::MouseDown { button, .. } if *button == MouseButton::Left => {
            set_open(ui, (field, popup), s, toggle);
        }
        Event::KeyDown { key, .. } => match *key {
            Key::RETURN | Key::SPACE => set_open(ui, (field, popup), s, toggle),
            Key::ESCAPE => set_open(ui, (field, popup), s, false),
            Key::UP | Key::DOWN => {
                let cur = s.selected.get();
                let max = len.saturating_sub(1);
                s.selected.set(match *key {
                    Key::DOWN => (cur + 1).min(max),
                    _ => cur.saturating_sub(1),
                });
                ui.invalidate(field);
            }
            _ => {}
        },
        _ => {}
    }
    None
}

/// Handles popup input: hover highlights, a left click chooses a row.
fn popup_event<M: 'static>(
    s: &Shared<M>,
    ui: &Ui<M>,
    field: WidgetId,
    popup: WidgetId,
    event: &Event,
) -> Option<M> {
    if (ui.is_design_mode() && event.is_input()) || !s.enabled.get() {
        return None;
    }
    if let Event::MouseMove { y, .. } = event {
        s.hover.set(row_at(ui, popup, *y, s.items.len()));
        ui.invalidate(popup);
        return None;
    }
    let Event::MouseDown { y, button, .. } = event else {
        return None;
    };
    if *button != MouseButton::Left {
        return None;
    }
    let index = row_at(ui, popup, *y, s.items.len())?;
    s.selected.set(index);
    set_open(ui, (field, popup), s, false);
    let mapper = s.on_select.borrow();
    mapper.as_ref().and_then(|mapper| mapper(index))
}

impl<M: 'static> ComboBox<M> {
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
        let top = backend.node(popup).unwrap().1.top;
        let row = ROW.to_px(ui.dpi()).value().max(1);
        rt.deliver(popup, &down(5, top + row * 2 + row / 2));
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
