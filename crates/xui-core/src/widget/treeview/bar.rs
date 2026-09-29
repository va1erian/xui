#![forbid(unsafe_code)]

//! The tree's vertical scrollbar: its metrics, layout and input mapping.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use super::flatten::{self, State};
use crate::app::Ui;
use crate::backend::{Event, WidgetId};
use crate::geometry::Rect;
use crate::widget::scrollbar::{self, Scroll, ScrollBar};

/// The scroll metrics of the tree's body: every visible row is content.
pub(crate) fn metrics<M: 'static>(ui: &Ui<M>, id: WidgetId, state: &State) -> Scroll {
    let dpi = ui.dpi();
    Scroll {
        viewport: ui.bounds(id).height().max(0),
        content: state.visible_len() as i32 * row_px(dpi),
        offset: state.offset as i32 * row_px(dpi),
    }
}

/// Lays the bar along the trailing edge and shows it only on overflow.
pub(crate) fn layout<M: 'static>(ui: &Ui<M>, id: WidgetId, bar: &ScrollBar, state: &State) {
    let bounds = ui.bounds(id);
    let dpi = ui.dpi();
    let overflows = state.visible_len() as i32 * row_px(dpi) > bounds.height().max(0);
    let width = if overflows {
        scrollbar::THICKNESS.to_px(dpi).value()
    } else {
        0
    };
    let (w, h) = (bounds.width(), bounds.height());
    bar.set_track(width, h);
    ui.set_visible(bar.id(), overflows);
    ui.apply_moves(&[(bar.id(), Rect::new(w - width, 0, w, h))]);
}

/// Builds the event closure the bar node registers.
pub(crate) fn mapper<M: 'static>(
    ui: Ui<M>,
    id: WidgetId,
    bar: Rc<ScrollBar>,
    state: Rc<RefCell<State>>,
    enabled: Rc<Cell<bool>>,
) -> impl Fn(&Event) -> Option<M> + 'static {
    move |event| {
        if (ui.is_design_mode() && event.is_input()) || !enabled.get() {
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

/// One row's device-pixel height.
fn row_px(dpi: u32) -> i32 {
    flatten::ROW.to_px(dpi).value().max(1)
}

/// Applies a pixel `target` offset as a visible-slot offset, clamped.
fn set_offset<M: 'static>(
    ui: &Ui<M>,
    id: WidgetId,
    bar: &ScrollBar,
    state: &Rc<RefCell<State>>,
    target: i32,
    dpi: u32,
) {
    let visible = (ui.bounds(id).height().max(0) / row_px(dpi)) as usize;
    let mut state = state.borrow_mut();
    let max = state.visible_len().saturating_sub(visible.max(1));
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
    use std::rc::Rc;

    use super::*;
    use crate::app::{App, Core, Runtime};
    use crate::backend::headless::HeadlessBackend;
    use crate::backend::{Backend, PlatformSpec};
    use crate::geometry::Rect;
    use crate::message::{Modifiers, MouseButton};
    use crate::widget::treeview::{TreeRow, TreeView};

    struct TestApp;

    impl App for TestApp {
        type Msg = u32;
        fn update(&mut self, _msg: u32, _ui: &mut Ui<u32>) {}
    }

    fn setup() -> (Rc<HeadlessBackend>, Rc<Runtime<TestApp>>, Ui<u32>) {
        let backend = Rc::new(HeadlessBackend::new());
        let window = backend.open_window(&PlatformSpec::new("bar")).unwrap();
        let core = Core::new(backend.clone(), window);
        let ui = Ui::new(Rc::clone(&core));
        let runtime = Runtime::primary(core, TestApp);
        (backend, runtime, ui)
    }

    fn leaves(count: usize) -> Vec<TreeRow> {
        (0..count)
            .map(|index| TreeRow::new(format!("row {index}"), 0))
            .collect()
    }

    fn thumb(tree: &TreeView<u32>, ui: &Ui<u32>) -> Option<Rect> {
        let metrics = metrics(ui, tree.id(), &tree.state.borrow());
        tree.bar.thumb(metrics, ui.dpi())
    }

    #[test]
    fn the_bar_appears_only_when_the_rows_overflow() {
        let (backend, _runtime, ui) = setup();
        let short = TreeView::new(&ui, Rect::new(0, 0, 120, 88), &leaves(3)).unwrap();
        assert!(
            !backend.node(short.bar.id()).unwrap().3,
            "three rows fit, so no bar"
        );

        let long = TreeView::new(&ui, Rect::new(0, 0, 120, 88), &leaves(100)).unwrap();
        assert!(
            backend.node(long.bar.id()).unwrap().3,
            "a hundred rows overflow, so the bar shows"
        );
    }

    #[test]
    fn the_thumb_tracks_the_offset() {
        let (_backend, runtime, ui) = setup();
        let tree = TreeView::new(&ui, Rect::new(0, 0, 120, 88), &leaves(100)).unwrap();

        let top = thumb(&tree, &ui).expect("a thumb");
        runtime.deliver(
            tree.id(),
            &Event::MouseWheel {
                delta: -5,
                horizontal: false,
                x: 4,
                y: 4,
                modifiers: Modifiers::NONE,
            },
        );
        let moved = thumb(&tree, &ui).expect("a thumb");
        assert!(tree.state.borrow().offset > 0, "the wheel scrolls the tree");
        assert!(moved.top > top.top, "the thumb follows the scrolled offset");
        assert_eq!(top.height(), moved.height(), "its height is unchanged");
    }

    #[test]
    fn a_drag_scrolls_the_tree() {
        let (backend, runtime, ui) = setup();
        let tree = TreeView::new(&ui, Rect::new(0, 0, 120, 88), &leaves(100)).unwrap();

        let start = thumb(&tree, &ui).expect("a thumb");
        let start = start.top + start.height() / 2;
        runtime.deliver(
            tree.bar.id(),
            &Event::MouseDown {
                x: 112,
                y: start,
                button: MouseButton::Left,
                modifiers: Modifiers::NONE,
            },
        );
        assert_eq!(
            backend.captured(),
            Some(tree.bar.id()),
            "the thumb drag captures the pointer"
        );
        runtime.deliver(
            tree.bar.id(),
            &Event::MouseMove {
                x: 112,
                y: start + 40,
                modifiers: Modifiers::NONE,
            },
        );
        assert!(
            tree.state.borrow().offset > 0,
            "dragging the thumb down scrolls the tree"
        );
        runtime.deliver(
            tree.bar.id(),
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
