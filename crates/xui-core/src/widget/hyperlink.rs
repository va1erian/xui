#![forbid(unsafe_code)]

//! [`Hyperlink`]: a clickable link label.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use super::control::{Control, HasText};
use crate::app::Ui;
use crate::backend::{Cursor, Event, NodeKind, NodeSpec, Result, TextStyle};
use crate::geometry::{Point, Rect};
use crate::message::MouseButton;
use crate::property::{Properties, Property, Value};
use crate::units::Dip;

/// Maps a click to an optional app message.
type ClickMapper<M> = Rc<RefCell<Option<Box<dyn Fn() -> Option<M>>>>>;

/// The design size of the link text.
const TEXT_SIZE: Dip = Dip(12.0);
/// The width of the hover underline, in device pixels.
const UNDERLINE: f32 = 1.0;

/// A clickable link label.
///
/// The text is painted in the theme's accent colour and underlined while
/// hovered. Its click is mapped to the app's `Msg` through
/// [`Hyperlink::on_click`]; a disabled link is dimmed and ignores input.
pub struct Hyperlink<M: 'static> {
    control: Control<M>,
    text: Rc<RefCell<String>>,
    hovered: Rc<Cell<bool>>,
    enabled: Rc<Cell<bool>>,
    on_click: ClickMapper<M>,
}

impl<M: 'static> Hyperlink<M> {
    /// Creates a hyperlink with no bounds of its own, for a layout to place (see
    /// [`crate::arrange`]); its size comes from [`Placeable`](super::Placeable).
    pub fn auto(ui: &Ui<M>, text: &str) -> Result<Hyperlink<M>> {
        Hyperlink::new(ui, Rect::default(), text)
    }

    /// Creates a link labelled `text` at `bounds`.
    pub fn new(ui: &Ui<M>, bounds: Rect, text: &str) -> Result<Hyperlink<M>> {
        let control = Control::new(ui, &NodeSpec::new(NodeKind::Label, bounds).text(text))?;
        ui.set_cursor(control.id(), Cursor::Hand);
        let state = Rc::new(RefCell::new(text.to_string()));
        let hovered = Rc::new(Cell::new(false));
        let enabled = Rc::new(Cell::new(true));
        let on_click: ClickMapper<M> = Rc::new(RefCell::new(None));

        {
            let text = Rc::clone(&state);
            let hovered = Rc::clone(&hovered);
            let enabled = Rc::clone(&enabled);
            let theme = ui.theme_handle();
            let selected = control.selected_handle();
            control.set_painter(Rc::new(move |canvas| {
                let theme = theme.get();
                let bounds = canvas.bounds();
                let dpi = canvas.dpi();
                // Paint the opaque node's background first, or the back buffer
                // shows through around the glyphs.
                canvas.clear(theme.background);
                let color = if enabled.get() {
                    theme.accent
                } else {
                    theme.text_disabled
                };
                let style = TextStyle::new(color, TEXT_SIZE).middle();
                canvas.draw_text(&text.borrow(), bounds, &style);
                if hovered.get() {
                    // The canvas has no text measure; estimate the advance the
                    // way the other painted widgets do.
                    let size = TEXT_SIZE.to_px(dpi).value() as f32;
                    let width = (text.borrow().chars().count() as f32 * size * 0.5).round() as i32;
                    let baseline = bounds.top + bounds.height() / 2 + (size * 0.625).round() as i32;
                    canvas.draw_line(
                        Point::new(bounds.left, baseline),
                        Point::new(bounds.left + width, baseline),
                        color,
                        UNDERLINE,
                    );
                }
                if selected.get() {
                    canvas.stroke_rect(bounds, theme.accent, 2.0);
                }
            }));
        }

        {
            let hovered = Rc::clone(&hovered);
            let enabled = Rc::clone(&enabled);
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
                match event {
                    Event::MouseMove { .. } => {
                        if !hovered.replace(true) {
                            ui.invalidate(id);
                        }
                    }
                    Event::MouseLeave | Event::CaptureChanged => {
                        if hovered.replace(false) {
                            ui.invalidate(id);
                        }
                    }
                    Event::MouseUp {
                        button: MouseButton::Left,
                        ..
                    } => {
                        let mapper = on_click.borrow();
                        if let Some(mapper) = mapper.as_ref() {
                            return mapper();
                        }
                    }
                    _ => {}
                }
                None
            });
        }

        Ok(Hyperlink {
            control,
            text: state,
            hovered,
            enabled,
            on_click,
        })
    }

    /// Maps a click to the app's message: the closure returns `Some(msg)` to
    /// raise it, or `None` to ignore the click.
    pub fn on_click(self, mapper: impl Fn() -> Option<M> + 'static) -> Hyperlink<M> {
        *self.on_click.borrow_mut() = Some(Box::new(mapper));
        self
    }

    /// The link's node identity.
    pub fn id(&self) -> crate::backend::WidgetId {
        self.control.id()
    }

    /// Enables or disables the link. A disabled link is dimmed and ignores
    /// input.
    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.set(enabled);
        if !enabled {
            self.hovered.set(false);
        }
        self.control.set_enabled(enabled);
        self.control.invalidate();
    }

    /// Marks the link selected, so its painter draws an outline (a form
    /// editor's selection).
    pub fn set_selected(&self, selected: bool) {
        self.control.set_selected(selected);
    }
}

impl<M: 'static> HasText for Hyperlink<M> {
    fn text(&self) -> String {
        self.text.borrow().clone()
    }

    fn set_text(&self, text: &str) {
        *self.text.borrow_mut() = text.to_string();
        self.control.invalidate();
    }
}

impl<M: 'static> Properties for Hyperlink<M> {
    fn properties(&self) -> Vec<Property> {
        vec![Property {
            name: "text",
            value: Value::Text(self.text()),
        }]
    }

    fn set_property(&self, name: &str, value: Value) -> bool {
        match (name, value) {
            ("text", Value::Text(text)) => {
                self.set_text(&text);
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

    use super::Hyperlink;
    use crate::app::{App, Core, Runtime, Ui};
    use crate::backend::headless::{DrawOp, HeadlessBackend};
    use crate::backend::{Backend, Event, PlatformSpec, WidgetId};
    use crate::geometry::Rect;
    use crate::message::{Modifiers, MouseButton};
    use crate::widget::HasText;

    struct TestApp {
        log: Rc<RefCell<Vec<u32>>>,
    }

    impl App for TestApp {
        type Msg = u32;

        fn update(&mut self, msg: u32, _ui: &mut Ui<u32>) {
            self.log.borrow_mut().push(msg);
        }
    }

    fn setup() -> (Rc<HeadlessBackend>, Rc<Core<u32>>, Ui<u32>) {
        let backend = Rc::new(HeadlessBackend::new());
        let window = backend.open_window(&PlatformSpec::new("t")).unwrap();
        let core = Core::new(backend.clone(), window);
        let ui = Ui::new(Rc::clone(&core));
        (backend, core, ui)
    }

    #[test]
    fn a_left_click_raises_the_msg() {
        let (_backend, core, ui) = setup();
        let link = Hyperlink::new(&ui, Rect::new(0, 0, 120, 20), "docs")
            .unwrap()
            .on_click(|| Some(9));
        let log = Rc::new(RefCell::new(Vec::new()));
        let runtime = Runtime::primary(
            core,
            TestApp {
                log: Rc::clone(&log),
            },
        );

        runtime.deliver(
            link.id(),
            &Event::MouseUp {
                x: 5,
                y: 5,
                button: MouseButton::Left,
                modifiers: Modifiers::NONE,
            },
        );
        runtime.deliver(WidgetId::NONE, &Event::Wake);
        assert_eq!(*log.borrow(), vec![9]);
    }

    #[test]
    fn setting_text_repaints_the_link() {
        let (backend, _core, ui) = setup();
        let link = Hyperlink::new(&ui, Rect::new(0, 0, 120, 20), "one").unwrap();
        link.set_text("two");
        assert_eq!(link.text(), "two");

        backend.render(link.id());
        let ops = backend.ops(link.id());
        assert!(
            ops.iter()
                .any(|op| matches!(op, DrawOp::Text(_, text, _) if text == "two")),
            "the new text was painted: {ops:?}"
        );
    }
}
