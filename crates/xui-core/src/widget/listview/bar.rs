#![forbid(unsafe_code)]

//! The list's vertical scrollbar: its metrics, layout and input mapping.

use std::cell::RefCell;
use std::rc::Rc;

use super::state::{ROW, State, header_px};
use crate::app::Ui;
use crate::backend::{Event, WidgetId};
use crate::geometry::Rect;
use crate::widget::scrollbar::{self, Scroll, ScrollBar};

/// The scroll metrics of the list's body: every row is content.
pub(crate) fn metrics<M: 'static>(ui: &Ui<M>, id: WidgetId, state: &State) -> Scroll {
    let bounds = ui.bounds(id);
    let dpi = ui.dpi();
    Scroll {
        viewport: body_height(state.has_header(), bounds.height(), dpi),
        content: state.len() as i32 * row_px(dpi),
        offset: state.offset as i32 * row_px(dpi),
    }
}

/// Lays the bar along the body's trailing edge and shows it only on overflow.
pub(crate) fn layout<M: 'static>(ui: &Ui<M>, id: WidgetId, bar: &ScrollBar, state: &State) {
    let bounds = ui.bounds(id);
    let dpi = ui.dpi();
    let header = header_px(state.has_header(), dpi);
    let body = body_height(state.has_header(), bounds.height(), dpi);
    let overflows = state.len() as i32 * row_px(dpi) > body;
    let width = if overflows {
        scrollbar::THICKNESS.to_px(dpi).value()
    } else {
        0
    };
    let (w, h) = (bounds.width(), bounds.height());
    bar.set_track(width, body);
    ui.set_visible(bar.id(), overflows);
    ui.apply_moves(&[(bar.id(), Rect::new(w - width, header, w, h))]);
}

/// Builds the event closure the bar node registers.
pub(crate) fn mapper<M: 'static>(
    ui: Ui<M>,
    id: WidgetId,
    bar: Rc<ScrollBar>,
    state: Rc<RefCell<State>>,
) -> impl Fn(&Event) -> Option<M> + 'static {
    move |event| {
        if (ui.is_design_mode() && event.is_input()) || !state.borrow().enabled {
            return None;
        }
        let dpi = ui.dpi();
        let metrics = metrics(&ui, id, &state.borrow());
        bar.handle(
            &ui,
            metrics,
            |target| set_offset(&ui, id, &bar, &state, target, dpi),
            event,
        );
        None
    }
}

/// The body's height in pixels, below the header.
fn body_height(has_header: bool, height: i32, dpi: u32) -> i32 {
    (height - header_px(has_header, dpi)).max(0)
}

/// One row's device-pixel height.
fn row_px(dpi: u32) -> i32 {
    ROW.to_px(dpi).value().max(1)
}

/// Applies a pixel `target` offset as a row offset, clamped to the rows.
fn set_offset<M: 'static>(
    ui: &Ui<M>,
    id: WidgetId,
    bar: &ScrollBar,
    state: &Rc<RefCell<State>>,
    target: i32,
    dpi: u32,
) {
    let visible = {
        let state = state.borrow();
        let body = body_height(state.has_header(), ui.bounds(id).height(), dpi);
        (body / row_px(dpi)) as usize
    };
    let mut state = state.borrow_mut();
    let max = state.len().saturating_sub(visible.max(1));
    let next = ((target / row_px(dpi)).max(0) as usize).min(max);
    if next != state.offset {
        state.offset = next;
        drop(state);
        ui.invalidate(id);
        ui.invalidate(bar.id());
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
    use crate::geometry::Rect;
    use crate::message::{Modifiers, MouseButton};
    use crate::widget::listview::ListView;

    struct TestApp(Rc<RefCell<Vec<u32>>>);

    impl App for TestApp {
        type Msg = u32;
        fn update(&mut self, msg: u32, _ui: &mut Ui<u32>) {
            self.0.borrow_mut().push(msg);
        }
    }

    fn setup() -> (Rc<HeadlessBackend>, Rc<Core<u32>>, Ui<u32>) {
        let backend = Rc::new(HeadlessBackend::new());
        let window = backend.open_window(&PlatformSpec::new("bar")).unwrap();
        let core = Core::new(backend.clone(), window);
        let ui = Ui::new(Rc::clone(&core));
        (backend, core, ui)
    }

    fn rows() -> Vec<String> {
        (0..100).map(|index| format!("row {index}")).collect()
    }

    fn thumb(list: &ListView<u32>, ui: &Ui<u32>) -> Option<Rect> {
        let metrics = metrics(ui, list.id(), &list.state.borrow());
        list.bar.thumb(metrics, ui.dpi())
    }

    #[test]
    fn the_bar_appears_only_when_the_rows_overflow() {
        let (backend, _core, ui) = setup();
        let short = ListView::new(&ui, Rect::new(0, 0, 120, 88), &["a", "b", "c"]).unwrap();
        assert!(
            !backend.node(short.bar.id()).unwrap().3,
            "three rows fit, so no bar"
        );

        let long = ListView::with_model(&ui, Rect::new(0, 0, 120, 88), rows()).unwrap();
        assert!(
            backend.node(long.bar.id()).unwrap().3,
            "a hundred rows overflow, so the bar shows"
        );
    }

    #[test]
    fn the_thumb_tracks_the_offset() {
        let (_backend, _core, ui) = setup();
        let list = ListView::with_model(&ui, Rect::new(0, 0, 120, 88), rows()).unwrap();

        let top = thumb(&list, &ui).expect("a thumb");
        list.ensure_visible(50);
        let moved = thumb(&list, &ui).expect("a thumb");
        assert!(moved.top > top.top, "the thumb follows the scrolled offset");
        assert_eq!(top.height(), moved.height(), "its height is unchanged");
    }

    #[test]
    fn a_drag_scrolls_the_list() {
        let (backend, core, ui) = setup();
        let list = ListView::with_model(&ui, Rect::new(0, 0, 120, 88), rows()).unwrap();
        let runtime = Runtime::primary(core, TestApp(Rc::new(RefCell::new(Vec::new()))));

        let start = thumb(&list, &ui).expect("a thumb");
        let start = start.top + start.height() / 2;
        runtime.deliver(
            list.bar.id(),
            &Event::MouseDown {
                x: 112,
                y: start,
                button: MouseButton::Left,
                modifiers: Modifiers::NONE,
            },
        );
        assert_eq!(
            backend.captured(),
            Some(list.bar.id()),
            "the thumb drag captures the pointer"
        );
        runtime.deliver(
            list.bar.id(),
            &Event::MouseMove {
                x: 112,
                y: start + 40,
                modifiers: Modifiers::NONE,
            },
        );
        assert!(
            list.state.borrow().offset > 0,
            "dragging the thumb down scrolls the list"
        );
        runtime.deliver(
            list.bar.id(),
            &Event::MouseUp {
                x: 112,
                y: start + 40,
                button: MouseButton::Left,
                modifiers: Modifiers::NONE,
            },
        );
        assert_eq!(backend.captured(), None, "the release drops the capture");
    }
}
