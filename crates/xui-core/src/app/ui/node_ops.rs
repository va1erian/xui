#![forbid(unsafe_code)]

//! [`Ui`](super::Ui)'s node and widget operations: create, destroy, move, show
//! and configure a node. Split from `ui.rs` so both files stay under the size
//! limit.

use crate::backend::{
    Cursor, ImplKind, NodeKind, NodeSpec, Painter, ParentRef, Result, TextMetrics, TextStyle,
    WidgetId,
};
use crate::geometry::Rect;

use super::Ui;

impl<M: 'static> Ui<M> {
    /// Creates a node from `spec`, parented to the container this handle is
    /// scoped to, or to the window.
    pub fn create_node(&self, spec: &NodeSpec) -> Result<WidgetId> {
        let parent = self
            .parent
            .map_or(ParentRef::Window(self.core.window()), ParentRef::Widget);
        self.core.backend().create(parent, spec)
    }

    /// Creates a node inside the container `parent`.
    pub fn create_child(&self, parent: WidgetId, spec: &NodeSpec) -> Result<WidgetId> {
        self.core.backend().create(ParentRef::Widget(parent), spec)
    }

    /// Destroys a node and every node inside it, and forgets its event mapper.
    ///
    /// Unregistering first matters: a mapper captures the `Ui` (and so the
    /// `Core`), and leaving it in the router would keep the whole window alive.
    pub fn destroy(&self, id: WidgetId) {
        self.core.router().unregister(id);
        self.core.set_hidden(id, false);
        self.core.backend().destroy(id);
    }

    /// Moves several nodes as one batch.
    pub fn apply_moves(&self, moves: &[(WidgetId, Rect)]) {
        self.core.backend().apply_moves(self.core.window(), moves);
    }

    /// Shows or hides a node. A mounted layout re-flows when this changes
    /// whether the node takes part in it: a hidden node takes no space.
    pub fn set_visible(&self, id: WidgetId, visible: bool) {
        self.core.backend().set_visible(id, visible);
        if self.core.set_hidden(id, !visible) {
            self.core.run_layout_hooks();
        }
    }

    /// Whether a node is shown, as last set through [`Ui::set_visible`].
    pub fn is_visible(&self, id: WidgetId) -> bool {
        !self.core.is_hidden(id)
    }

    /// Re-flows every mounted layout. Call it after a change that alters a
    /// widget's natural size (its text, say); window resizes, DPI changes and
    /// visibility changes re-flow on their own.
    pub fn relayout(&self) {
        self.core.run_layout_hooks();
    }

    /// Adds a relayout callback, returning the token that removes it.
    pub(crate) fn add_layout_hook(&self, f: impl Fn() + 'static) -> usize {
        self.core.add_layout_hook(f)
    }

    /// Removes a relayout callback added with [`Ui::add_layout_hook`].
    pub(crate) fn remove_layout_hook(&self, token: usize) {
        self.core.remove_layout_hook(token);
    }

    /// Enables or disables a node.
    pub fn set_enabled(&self, id: WidgetId, enabled: bool) {
        self.core.backend().set_enabled(id, enabled);
    }

    /// Raises a node above its siblings in the z-order.
    pub fn raise(&self, id: WidgetId) {
        self.core.backend().raise(id);
    }

    /// Requests the pointer shape shown over a node.
    pub fn set_cursor(&self, id: WidgetId, cursor: Cursor) {
        self.core.backend().set_cursor(id, cursor);
    }

    /// Clips a node's descendants to `rect`, in the node's own coordinates;
    /// `None` clears the clip.
    pub fn set_clip(&self, id: WidgetId, rect: Option<Rect>) {
        self.core.backend().set_clip(id, rect);
    }

    /// Routes subsequent pointer moves and releases to `id`, even outside it,
    /// so a drag that began on it survives the pointer leaving.
    pub fn set_capture(&self, id: WidgetId) {
        self.core.backend().set_capture(id);
    }

    /// Releases the pointer capture taken with [`Ui::set_capture`].
    pub fn release_capture(&self) {
        self.core.backend().release_capture();
    }

    /// Gives a node the keyboard focus.
    pub fn focus(&self, id: WidgetId) {
        self.core.backend().focus(id);
    }

    /// Replaces a node's text.
    pub fn set_text(&self, id: WidgetId, text: &str) {
        self.core.backend().set_text(id, text);
    }

    /// Sets a node's cue banner (placeholder shown while a field is empty);
    /// a painted field draws its own cue instead.
    pub fn set_cue(&self, id: WidgetId, cue: &str) {
        self.core.backend().set_cue(id, cue);
    }

    /// A node's current text (a native control answers from its own state).
    pub fn text(&self, id: WidgetId) -> String {
        self.core.backend().text(id)
    }

    /// A node's current bounds, in device pixels.
    pub fn bounds(&self, id: WidgetId) -> Rect {
        self.core.backend().bounds(id)
    }

    /// Whether the window is in design mode. In design mode a widget ignores its
    /// own input, so a form editor can select and move it.
    pub fn is_design_mode(&self) -> bool {
        self.core.design_mode()
    }

    /// Turns design mode on or off.
    pub fn set_design_mode(&self, on: bool) {
        self.core.set_design_mode(on);
    }

    /// Schedules a repaint of a node.
    pub fn invalidate(&self, id: WidgetId) {
        self.core.backend().invalidate(id);
    }

    /// Schedules a repaint of `rect` within a node.
    pub fn invalidate_rect(&self, id: WidgetId, rect: Rect) {
        self.core.backend().invalidate_rect(id, rect);
    }

    /// Registers the draw routine for a painted node.
    pub fn set_painter(&self, id: WidgetId, painter: Painter) {
        self.core.backend().set_painter(id, painter);
    }

    /// Measures a run of text in device pixels.
    pub fn measure_text(&self, text: &str, style: &TextStyle, dpi: u32) -> TextMetrics {
        self.core.backend().measure_text(text, style, dpi)
    }

    /// A `Send + Sync` text shaper for this window's backend, usable from a
    /// worker thread that lays text out off the UI thread.
    pub fn text_shaper(&self) -> Box<dyn crate::backend::TextShaper> {
        self.core.backend().text_shaper()
    }

    /// Whether the backend provides a native widget for `kind`.
    pub fn supports(&self, kind: NodeKind) -> ImplKind {
        self.core.backend().supports(kind)
    }
}
