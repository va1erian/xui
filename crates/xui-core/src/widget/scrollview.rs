#![forbid(unsafe_code)]

//! [`ScrollView`]: a container with a scrollable content area and a vertical
//! scrollbar.
//!
//! Content widgets are created through [`ScrollView::ui`] (so they parent to
//! the view) and registered with [`ScrollView::add`], which stacks them top to
//! bottom with the layout engine. Scrolling moves them as one batch and culls
//! those fully outside the viewport. The view owns its scrollbar child: a thumb
//! drag, the wheel and the arrow/Page/Home/End keys all scroll it, and
//! [`ScrollView::on_scroll`] maps the offset to the app's message.

#[cfg(test)]
mod tests;
mod view;

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use super::control::Control;
use crate::app::Ui;
use crate::backend::{NodeKind, NodeSpec, Result, WidgetId};
use crate::geometry::Rect;
use crate::layout::Stack;
use crate::property::{Properties, Property, Value};
use crate::units::{Dip, Px};
use view::{bar_event, paint_bar, paint_viewport, set_offset, viewport_event};

/// The scrollbar's width.
const BAR: Dip = Dip(12.0);
/// How far one wheel notch scrolls.
const WHEEL_STEP: Dip = Dip(48.0);
/// How far an arrow key scrolls.
const LINE_STEP: Dip = Dip(24.0);
/// The shortest the thumb may shrink to.
const MIN_THUMB: Dip = Dip(24.0);

type ScrollMapper<M> = RefCell<Option<Box<dyn Fn(Px) -> Option<M>>>>;

/// A registered content row: a widget and the design height it occupies.
struct Row {
    id: WidgetId,
    height: Dip,
}

/// State the view, its scrollbar and their painters and mappers share.
struct Shared<M: 'static> {
    id: WidgetId,
    bar_id: WidgetId,
    /// The content viewport in the view's own coordinates (excludes the bar).
    viewport: Cell<Rect>,
    /// The scrollbar's rectangle in the view's own coordinates.
    bar: Cell<Rect>,
    /// The content's total height in pixels.
    content: Cell<i32>,
    /// The scroll offset in pixels.
    offset: Cell<i32>,
    rows: RefCell<Vec<Row>>,
    /// The pointer coordinate a thumb drag started at, while dragging.
    drag: Cell<Option<i32>>,
    /// The offset when the drag started.
    drag_offset: Cell<i32>,
    on_scroll: ScrollMapper<M>,
}

/// A container with a scrollable content area and a vertical scrollbar.
pub struct ScrollView<M: 'static> {
    control: Control<M>,
    _bar: Control<M>,
    scoped: Ui<M>,
    shared: Rc<Shared<M>>,
}

impl<M: 'static> ScrollView<M> {
    /// Creates a scroll view at `bounds`.
    pub fn new(ui: &Ui<M>, bounds: Rect) -> Result<ScrollView<M>> {
        let control = Control::new(ui, &NodeSpec::new(NodeKind::ScrollView, bounds).tab_stop())?;
        let scoped = ui.with_parent(control.id());
        let bar = Control::new(
            &scoped,
            &NodeSpec::new(NodeKind::Container, Rect::default()),
        )?;
        let shared = Rc::new(Shared {
            id: control.id(),
            bar_id: bar.id(),
            viewport: Cell::new(bounds),
            bar: Cell::new(Rect::default()),
            content: Cell::new(0),
            offset: Cell::new(0),
            rows: RefCell::new(Vec::new()),
            drag: Cell::new(None),
            drag_offset: Cell::new(0),
            on_scroll: RefCell::new(None),
        });
        {
            let theme = ui.theme_handle();
            control.set_painter(Rc::new(move |canvas| paint_viewport(canvas, theme.get())));
        }
        {
            let shared = Rc::clone(&shared);
            let theme = ui.theme_handle();
            bar.set_painter(Rc::new(move |canvas| {
                paint_bar(&shared, canvas, theme.get())
            }));
        }
        {
            let shared = Rc::clone(&shared);
            let ui = ui.clone();
            control.on_events(move |event| viewport_event(&shared, &ui, event));
        }
        {
            let shared = Rc::clone(&shared);
            let ui = ui.clone();
            bar.on_events(move |event| bar_event(&shared, &ui, event));
        }
        ui.raise(shared.bar_id);
        Ok(ScrollView {
            control,
            _bar: bar,
            scoped,
            shared,
        })
    }

    /// The handle widgets built inside this view parent to.
    pub fn ui(&self) -> &Ui<M> {
        &self.scoped
    }

    /// The view's node identity.
    pub fn id(&self) -> WidgetId {
        self.control.id()
    }

    /// Adds `id` as a content row `height` design values tall.
    pub fn add(&self, id: WidgetId, height: Dip) {
        self.shared.rows.borrow_mut().push(Row { id, height });
        relayout(&self.scoped, &self.shared);
        self.scoped.raise(self.shared.bar_id);
    }

    /// The content's total height.
    pub fn content_height(&self) -> Px {
        Px(self.shared.content.get())
    }

    /// The current scroll offset.
    pub fn offset(&self) -> Px {
        Px(self.shared.offset.get())
    }

    /// Scrolls to `offset`, clamped to the content.
    pub fn scroll_to(&self, offset: Px) {
        set_offset(&self.scoped, &self.shared, offset.value());
    }

    /// Maps a scroll to the app's message: the closure receives the new offset
    /// and returns `Some(msg)` to raise it, or `None` to ignore it.
    pub fn on_scroll(self, f: impl Fn(Px) -> Option<M> + 'static) -> ScrollView<M> {
        *self.shared.on_scroll.borrow_mut() = Some(Box::new(f));
        self
    }

    /// Moves/resizes the view and re-lays its content out.
    pub fn set_bounds(&self, bounds: Rect) {
        self.control.set_bounds(bounds);
        relayout(&self.scoped, &self.shared);
    }

    /// Re-lays the content out from the view's current bounds.
    pub fn relayout(&self) {
        relayout(&self.scoped, &self.shared);
    }

    /// Shows or hides the view.
    pub fn set_visible(&self, visible: bool) {
        self.control.set_visible(visible);
    }

    /// Enables or disables the view.
    pub fn set_enabled(&self, enabled: bool) {
        self.control.set_enabled(enabled);
    }
}

impl<M: 'static> Properties for ScrollView<M> {
    fn properties(&self) -> Vec<Property> {
        vec![Property {
            name: "offset",
            value: Value::Integer(i64::from(self.offset().value())),
        }]
    }

    fn set_property(&self, name: &str, value: Value) -> bool {
        match (name, value) {
            ("offset", Value::Integer(px)) => {
                self.scroll_to(Px(px as i32));
                true
            }
            _ => false,
        }
    }
}

/// Lays every row out in the viewport at the current offset and culls the rows
/// fully outside it.
fn relayout<M>(ui: &Ui<M>, s: &Shared<M>) {
    let node = ui.bounds(s.id);
    if node.is_empty() {
        return;
    }
    // Children are parented to the view, so they are placed in its own
    // coordinates: start the viewport at the origin.
    let bounds = Rect::from_size(node.size());
    let dpi = ui.dpi();
    let content: i32 = s
        .rows
        .borrow()
        .iter()
        .map(|row| row.height.to_px(dpi).value().max(0))
        .sum();
    let overflows = content > bounds.height();
    let bar_width = if overflows { BAR.to_px(dpi).value() } else { 0 };
    let viewport = Rect::new(
        bounds.left,
        bounds.top,
        (bounds.right - bar_width).max(bounds.left),
        bounds.bottom,
    );
    let bar = Rect::new(viewport.right, bounds.top, bounds.right, bounds.bottom);
    let max_offset = (content - viewport.height()).max(0);
    let offset = s.offset.get().clamp(0, max_offset);
    s.content.set(content);
    s.viewport.set(viewport);
    s.bar.set(bar);
    s.offset.set(offset);
    ui.set_visible(s.bar_id, overflows);

    let rows = s.rows.borrow();
    let mut stack = Stack::vertical();
    for row in rows.iter() {
        stack = stack.fixed(row.height);
    }
    let content_rect = Rect::new(
        viewport.left,
        viewport.top,
        viewport.right,
        viewport.top + content.max(viewport.height()),
    );
    let rects = stack.split(content_rect, dpi);

    let mut moves: Vec<(WidgetId, Rect)> = Vec::with_capacity(rows.len() + 1);
    if overflows {
        moves.push((s.bar_id, bar));
    }
    for (row, rect) in rows.iter().zip(rects) {
        let rect = rect.offset(0, -offset);
        ui.set_visible(
            row.id,
            rect.bottom > viewport.top && rect.top < viewport.bottom,
        );
        moves.push((row.id, rect));
    }
    ui.apply_moves(&moves);
    ui.invalidate(s.bar_id);
}
