#![forbid(unsafe_code)]

//! [`TreeView`]: a flattened, fixed-row-height collapsible tree.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use super::control::Control;
use crate::app::Ui;
use crate::backend::{Event, NodeKind, NodeSpec, Result, TextStyle, WidgetId};
use crate::geometry::{Point, Rect};
use crate::message::{Key, MouseButton};
use crate::property::{Properties, Property, Value};
use crate::units::Dip;

type SelectMapper<M> = Rc<RefCell<Option<Box<dyn Fn(usize) -> Option<M>>>>>;
type ToggleMapper<M> = Rc<RefCell<Option<Box<dyn Fn(usize, bool) -> Option<M>>>>>;

const ROW: Dip = Dip(22.0);

/// One flattened tree row, in display order.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TreeRow {
    /// The row's label.
    pub label: String,
    /// The row's indent depth (`0` for a root).
    pub depth: u16,
    /// Whether the row has children and can be collapsed.
    pub expandable: bool,
    /// Whether the row is expanded.
    pub expanded: bool,
}

impl TreeRow {
    /// A collapsed, non-expandable row of `depth`.
    pub fn new(label: impl Into<String>, depth: u16) -> TreeRow {
        TreeRow {
            label: label.into(),
            depth,
            ..Default::default()
        }
    }

    /// Sets whether the row can be collapsed.
    pub fn expandable(mut self, expandable: bool) -> TreeRow {
        self.expandable = expandable;
        self
    }

    /// Sets whether the row starts expanded.
    pub fn expanded(mut self, expanded: bool) -> TreeRow {
        self.expanded = expanded;
        self
    }
}

fn row_at(dpi: u32, y: i32, count: usize) -> Option<usize> {
    if y < 0 {
        return None;
    }
    let index = (y / ROW.to_px(dpi).value().max(1)) as usize;
    (index < count).then_some(index)
}

fn chevron_hit(dpi: u32, row: &TreeRow, x: i32) -> bool {
    let left = Dip(4.0).to_px(dpi).value() + row.depth as i32 * Dip(16.0).to_px(dpi).value();
    row.expandable && x >= left && x < left + Dip(16.0).to_px(dpi).value()
}

/// Whether the row at `index` is shown: every ancestor row (the nearest
/// preceding row at each smaller depth) must be expanded.
fn is_visible(rows: &[TreeRow], index: usize) -> bool {
    let mut depth = rows[index].depth;
    let mut i = index;
    while depth > 0 {
        let parent_depth = depth - 1;
        let mut parent = None;
        while i > 0 {
            i -= 1;
            if rows[i].depth == parent_depth {
                parent = Some(i);
                break;
            }
            if rows[i].depth < parent_depth {
                break;
            }
        }
        match parent {
            Some(p) if rows[p].expandable && rows[p].expanded => depth = parent_depth,
            _ => return false,
        }
    }
    true
}

/// The raw row index drawn at visible slot `slot`, or `None` past the end.
fn slot_to_index(rows: &[TreeRow], slot: usize) -> Option<usize> {
    let mut visible = 0;
    for index in 0..rows.len() {
        if !is_visible(rows, index) {
            continue;
        }
        if visible == slot {
            return Some(index);
        }
        visible += 1;
    }
    None
}

/// A flattened, fixed-row-height collapsible tree, rows supplied in order.
pub struct TreeView<M: 'static> {
    control: Control<M>,
    rows: Rc<RefCell<Vec<TreeRow>>>,
    selected: Rc<Cell<Option<usize>>>,
    hover: Rc<Cell<Option<usize>>>,
    enabled: Rc<Cell<bool>>,
    on_select: SelectMapper<M>,
    on_toggle: ToggleMapper<M>,
}

impl<M: 'static> TreeView<M> {
    /// Creates a tree of `rows` at `bounds`.
    pub fn new(ui: &Ui<M>, bounds: Rect, rows: &[TreeRow]) -> Result<TreeView<M>> {
        let control = Control::new(ui, &NodeSpec::new(NodeKind::TreeView, bounds).tab_stop())?;
        let rows = Rc::new(RefCell::new(rows.to_vec()));
        let selected = Rc::new(Cell::new(None));
        let hover = Rc::new(Cell::new(None));
        let enabled = Rc::new(Cell::new(true));
        let on_select: SelectMapper<M> = Rc::new(RefCell::new(None));
        let on_toggle: ToggleMapper<M> = Rc::new(RefCell::new(None));
        {
            let rows = Rc::clone(&rows);
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
                let pad = Dip(4.0).to_px(dpi).value();
                let indent = Dip(16.0).to_px(dpi).value();
                let chevron = Dip(16.0).to_px(dpi).value();
                let enabled = enabled.get();
                let rows = rows.borrow();
                let mut slot: i32 = 0;
                for (index, entry) in rows.iter().enumerate() {
                    if !is_visible(&rows, index) {
                        continue;
                    }
                    let top = bounds.top + row * slot;
                    slot += 1;
                    let rect = Rect::new(bounds.left, top, bounds.right, top + row);
                    let current = selected.get() == Some(index);
                    if current {
                        canvas.fill_rect(rect, theme.accent);
                    } else if hover.get() == Some(index) {
                        canvas.fill_rect(rect, theme.hover);
                    }
                    let color = match (enabled, current) {
                        (false, _) => theme.text_disabled,
                        (true, true) => theme.text_on_accent,
                        (true, false) => theme.text,
                    };
                    let base = bounds.left + pad + entry.depth as i32 * indent;
                    let left = if entry.expandable {
                        let (cx, cy) = (base + chevron / 2, top + row / 2);
                        let half = (chevron / 4).max(1);
                        let (tip, tail) = if entry.expanded {
                            (Point::new(cx, cy + half), Point::new(cx + half, cy - half))
                        } else {
                            (Point::new(cx + half, cy), Point::new(cx - half, cy + half))
                        };
                        canvas.draw_line(Point::new(cx - half, cy - half), tip, color, 1.5);
                        canvas.draw_line(tip, tail, color, 1.5);
                        base + chevron
                    } else {
                        base
                    };
                    let style = TextStyle::new(color, Dip(12.0)).middle();
                    let text = Rect::new(left, top, bounds.right - pad, top + row);
                    canvas.draw_text(&entry.label, text, &style);
                }
                if flag.get() {
                    canvas.stroke_rect(bounds, theme.accent, 2.0);
                }
            }));
        }
        {
            let rows = Rc::clone(&rows);
            let selected = Rc::clone(&selected);
            let hover = Rc::clone(&hover);
            let enabled = Rc::clone(&enabled);
            let on_select = Rc::clone(&on_select);
            let on_toggle = Rc::clone(&on_toggle);
            let ui = ui.clone();
            let id = control.id();
            control.on_events(move |event| {
                // In design mode the editor handles input, not the widget.
                if (ui.is_design_mode() && event.is_input()) || !enabled.get() {
                    return None;
                }
                let toggle = |i, e| on_toggle.borrow().as_ref().and_then(|map| map(i, e));
                let set_expanded = |index: usize, expanded: bool| {
                    let mut entries = rows.borrow_mut();
                    if !entries[index].expandable || entries[index].expanded == expanded {
                        return None;
                    }
                    entries[index].expanded = expanded;
                    drop(entries);
                    ui.invalidate(id);
                    toggle(index, expanded)
                };
                match event {
                    Event::MouseDown { button, .. } if *button == MouseButton::Left => {
                        let (x, y) = event.position()?;
                        let slot = row_at(ui.dpi(), y, rows.borrow().len())?;
                        let index = slot_to_index(&rows.borrow(), slot)?;
                        if chevron_hit(ui.dpi(), &rows.borrow()[index], x) {
                            let expanded = !rows.borrow()[index].expanded;
                            set_expanded(index, expanded)
                        } else {
                            selected.set(Some(index));
                            ui.invalidate(id);
                            on_select.borrow().as_ref().and_then(|map| map(index))
                        }
                    }
                    Event::MouseMove { y, .. } => {
                        let index = row_at(ui.dpi(), *y, rows.borrow().len())
                            .and_then(|slot| slot_to_index(&rows.borrow(), slot));
                        if hover.get() != index {
                            hover.set(index);
                            ui.invalidate(id);
                        }
                        None
                    }
                    Event::MouseLeave => {
                        hover.set(None);
                        ui.invalidate(id);
                        None
                    }
                    Event::KeyDown { repeat, system, .. } if *repeat <= 1 && !*system => {
                        let key = match *event {
                            Event::KeyDown { key, .. } => key,
                            _ => return None,
                        };
                        let len = rows.borrow().len();
                        if len == 0 {
                            return None;
                        }
                        let current = selected.get();
                        match key {
                            Key::UP | Key::DOWN => {
                                let entries = rows.borrow();
                                let step = if key == Key::UP { -1 } else { 1 };
                                let mut at = current.unwrap_or(0) as isize + step;
                                while at >= 0
                                    && (at as usize) < len
                                    && !is_visible(&entries, at as usize)
                                {
                                    at += step;
                                }
                                if at >= 0 && (at as usize) < len {
                                    selected.set(Some(at as usize));
                                    ui.invalidate(id);
                                }
                                None
                            }
                            Key::LEFT | Key::RIGHT => set_expanded(current?, key == Key::RIGHT),
                            Key::RETURN => {
                                on_select.borrow().as_ref().and_then(|map| map(current?))
                            }
                            _ => None,
                        }
                    }
                    _ => None,
                }
            });
        }
        Ok(TreeView {
            control,
            rows,
            selected,
            hover,
            enabled,
            on_select,
            on_toggle,
        })
    }

    /// Maps selecting a row to the app's message.
    pub fn on_select(self, mapper: impl Fn(usize) -> Option<M> + 'static) -> TreeView<M> {
        *self.on_select.borrow_mut() = Some(Box::new(mapper));
        self
    }

    /// Maps toggling a row and its new expanded state to the app's message.
    pub fn on_toggle(self, mapper: impl Fn(usize, bool) -> Option<M> + 'static) -> TreeView<M> {
        *self.on_toggle.borrow_mut() = Some(Box::new(mapper));
        self
    }

    /// The selected row, if any.
    pub fn selected(&self) -> Option<usize> {
        self.selected.get()
    }

    /// Selects `index` without raising an event; out of range clears it.
    pub fn select(&self, index: Option<usize>) {
        self.selected
            .set(index.filter(|index| *index < self.rows.borrow().len()));
        self.control.invalidate();
    }

    /// Replaces the rows; an out-of-range selection is cleared.
    pub fn set_rows(&self, rows: &[TreeRow]) {
        *self.rows.borrow_mut() = rows.to_vec();
        self.select(self.selected.get().filter(|index| *index < rows.len()));
        self.hover.set(None);
    }

    /// The number of rows.
    pub fn len(&self) -> usize {
        self.rows.borrow().len()
    }

    /// Whether the tree has no rows.
    pub fn is_empty(&self) -> bool {
        self.rows.borrow().is_empty()
    }

    /// The tree's node identity.
    pub fn id(&self) -> WidgetId {
        self.control.id()
    }

    /// Enables or disables the tree; a disabled tree is dimmed and ignores input.
    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.set(enabled);
        self.control.set_enabled(enabled);
        self.control.invalidate();
    }

    /// Marks the tree selected, so its painter draws an outline.
    pub fn set_selected(&self, selected: bool) {
        self.control.set_selected(selected);
    }
}

impl<M: 'static> Properties for TreeView<M> {
    fn properties(&self) -> Vec<Property> {
        vec![Property {
            name: "selected",
            value: Value::Integer(self.selected().map_or(-1, |index| index as i64)),
        }]
    }

    fn set_property(&self, name: &str, value: Value) -> bool {
        if name != "selected" {
            return false;
        }
        if let Value::Integer(index) = value {
            self.select((index >= 0).then_some(index as usize));
            return true;
        }
        false
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

    thread_local! {
        static LOG: RefCell<Vec<u32>> = const { RefCell::new(Vec::new()) };
    }

    struct TestApp;

    impl App for TestApp {
        type Msg = u32;
        fn update(&mut self, msg: u32, _ui: &mut Ui<u32>) {
            LOG.with(|log| log.borrow_mut().push(msg));
        }
    }

    fn harness() -> (Rc<Runtime<TestApp>>, TreeView<u32>) {
        LOG.with(|log| log.borrow_mut().clear());
        let backend = Rc::new(HeadlessBackend::new());
        let window = backend.open_window(&PlatformSpec::new("t")).unwrap();
        let core = Core::new(backend.clone(), window);
        let ui = Ui::new(Rc::clone(&core));
        let rows = [
            TreeRow::new("root", 0).expandable(true),
            TreeRow::new("leaf", 1),
        ];
        let tree = TreeView::new(&ui, Rect::new(0, 0, 120, 88), &rows)
            .unwrap()
            .on_select(|i| Some(i as u32))
            .on_toggle(|i, e| Some(100 + i as u32 * 2 + e as u32));
        (Runtime::primary(core, TestApp), tree)
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
        let (runtime, tree) = harness();
        runtime.deliver(tree.id(), &down(80, 5));
        runtime.deliver(WidgetId::NONE, &Event::Wake);
        assert_eq!(tree.selected(), Some(0));
        assert_eq!(LOG.with(|log| log.borrow().clone()), vec![0]);
    }

    #[test]
    fn clicking_a_chevron_toggles_and_raises_a_message() {
        let (runtime, tree) = harness();
        runtime.deliver(tree.id(), &down(10, 11));
        runtime.deliver(tree.id(), &down(10, 11));
        runtime.deliver(WidgetId::NONE, &Event::Wake);
        assert_eq!(tree.selected(), None, "a chevron click does not select");
        assert_eq!(LOG.with(|log| log.borrow().clone()), vec![101, 100]);
    }

    #[test]
    fn a_collapsed_branch_hides_its_descendants() {
        let mut rows = [
            TreeRow::new("Inbox", 0).expandable(true).expanded(true),
            TreeRow::new("Work", 1).expandable(true).expanded(true),
            TreeRow::new("Deep", 2),
            TreeRow::new("Archive", 0).expandable(true),
            TreeRow::new("Old", 1),
        ];
        assert!(is_visible(&rows, 2), "an expanded chain shows the leaf");
        assert!(!is_visible(&rows, 4), "a collapsed parent hides its child");
        assert_eq!(slot_to_index(&rows, 2), Some(2));
        assert_eq!(slot_to_index(&rows, 3), Some(3));

        rows[0].expanded = false;
        assert!(!is_visible(&rows, 1), "collapsing hides the child");
        assert!(!is_visible(&rows, 2), "collapsing hides the grandchild");
        assert_eq!(slot_to_index(&rows, 1), Some(3), "Archive moves up a slot");
        assert_eq!(slot_to_index(&rows, 2), None, "no third visible row");
    }

    #[test]
    fn the_arrow_keys_expand_then_collapse_a_selected_row() {
        let (runtime, tree) = harness();
        tree.select(Some(0));
        runtime.deliver(tree.id(), &key(Key::RIGHT));
        runtime.deliver(tree.id(), &key(Key::LEFT));
        runtime.deliver(WidgetId::NONE, &Event::Wake);
        assert_eq!(LOG.with(|log| log.borrow().clone()), vec![101, 100]);
    }
}
