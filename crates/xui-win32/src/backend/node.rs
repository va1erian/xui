#![forbid(unsafe_code)]

//! One node the Win32 backend created, and how to create it: a painted child
//! window, or a native control where the kind has one.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use xui_core::Rect;
use xui_core::backend::{
    BackendError, ImplKind, NodeKind, NodeSpec, Painter, ParentRef, Result as BackendResult,
    WidgetId, WindowId,
};

use super::handler::{NodeHandler, WindowShared};
use crate::hwnd::Hwnd;
use crate::sys;
use crate::window::{Window, WindowClass, WindowExStyle, WindowStyle};

/// A node the backend created.
pub(super) struct BackendNode {
    /// A custom child window, kept alive so dropping the node destroys it.
    /// `None` for a native control, which has no [`Window`] wrapper.
    _window: Option<Window>,
    pub(super) window_id: WindowId,
    pub(super) parent: ParentRef,
    pub(super) hwnd: Hwnd,
    kind: NodeKind,
    /// Text of a custom node (a native control answers from its own state).
    text: RefCell<String>,
    /// The last bounds the layout assigned.
    bounds: Cell<Rect>,
    pub(super) painter: Rc<RefCell<Option<Painter>>>,
    /// Whether a press that starts on the node drags the whole window.
    pub(super) drag_region: Rc<Cell<bool>>,
}

impl Drop for BackendNode {
    fn drop(&mut self) {
        // Destroys a native control (and, harmlessly, a custom window that its
        // own `Window` also destroys).
        sys::window::destroy(self.hwnd);
    }
}

impl BackendNode {
    /// Creates the node for `spec`, parented under `parent_hwnd`.
    pub(super) fn create(
        window_id: WindowId,
        parent_hwnd: Hwnd,
        parent: ParentRef,
        shared: &Rc<WindowShared>,
        widget: WidgetId,
        spec: &NodeSpec,
    ) -> BackendResult<BackendNode> {
        let painter = Rc::new(RefCell::new(None));
        let drag_region = Rc::new(Cell::new(false));
        let (hwnd, window) = if spec.kind == NodeKind::Edit {
            create_native_edit(parent_hwnd, spec, shared.theme())?
        } else {
            create_painted(widget, parent_hwnd, shared, &painter, &drag_region, spec)?
        };

        if !spec.visible {
            sys::window::show(hwnd, sys::window::ShowKind::Hidden);
        }
        if !spec.enabled {
            sys::window::enable_window(hwnd, false);
        }
        shared.register_node(hwnd, widget);
        Ok(BackendNode {
            _window: window,
            window_id,
            parent,
            hwnd,
            kind: spec.kind,
            text: RefCell::new(spec.text.clone()),
            bounds: Cell::new(spec.bounds),
            painter,
            drag_region,
        })
    }

    /// The node's last assigned bounds.
    pub(super) fn bounds(&self) -> Rect {
        self.bounds.get()
    }

    /// Records the bounds the layout assigned.
    pub(super) fn set_bounds(&self, rect: Rect) {
        self.bounds.set(rect);
    }

    /// Whether the backend hosts this kind natively.
    pub(super) fn impl_kind(kind: NodeKind) -> ImplKind {
        match kind {
            NodeKind::Edit => ImplKind::Native,
            _ => ImplKind::Painted,
        }
    }

    /// The node's current text.
    pub(super) fn text(&self) -> String {
        if self.kind == NodeKind::Edit {
            sys::window::get_text(self.hwnd)
        } else {
            self.text.borrow().clone()
        }
    }

    /// Replaces the node's text.
    pub(super) fn set_text(&self, text: &str) {
        let _ = sys::window::set_title(self.hwnd, text);
        *self.text.borrow_mut() = text.to_string();
    }
}

/// A real `EDIT` control: it edits itself (IME included) and reports changes to
/// its parent.
///
/// A plain `WS_BORDER` replaces the classic `WS_EX_CLIENTEDGE` bevel, which is
/// light regardless of the theme; [`sys::edit_edge`] then strokes that one-pixel
/// frame with `theme.border`/`theme.border_focused`, matching the portable
/// `Edit`'s painted border.
fn create_native_edit(
    parent_hwnd: Hwnd,
    spec: &NodeSpec,
    theme: crate::theme::Theme,
) -> BackendResult<(Hwnd, Option<Window>)> {
    use crate::controls::style::{WS_BORDER, WS_CHILD, WS_TABSTOP, WS_VISIBLE};
    let mut style = WS_CHILD | WS_VISIBLE | WS_BORDER;
    if spec.tab_stop {
        style |= WS_TABSTOP;
    }
    let id = crate::controls::next_id();
    let hwnd =
        crate::controls::create_child("Edit", "EDIT", parent_hwnd, style, 0, id, spec.bounds)
            .map_err(|_| BackendError::CreateFailed("edit"))?;
    sys::edit_edge::install(hwnd, theme);
    if !spec.text.is_empty() {
        let _ = sys::window::set_title(hwnd, &spec.text);
    }
    Ok((hwnd, None))
}

/// A painted child window the front layer draws into.
fn create_painted(
    widget: WidgetId,
    parent_hwnd: Hwnd,
    shared: &Rc<WindowShared>,
    painter: &Rc<RefCell<Option<Painter>>>,
    drag_region: &Rc<Cell<bool>>,
    spec: &NodeSpec,
) -> BackendResult<(Hwnd, Option<Window>)> {
    let bounds = Rc::new(Cell::new(Rect::from_size(spec.bounds.size())));
    let handler = NodeHandler::new(
        widget,
        Rc::clone(shared),
        Rc::clone(&bounds),
        Rc::clone(painter),
        Rc::clone(drag_region),
    );
    let class = WindowClass::register("xui.node", xui_core::Theme::light().background)
        .map_err(|_| BackendError::CreateFailed("node class"))?;
    let mut style = WindowStyle::new().child().visible().clip_siblings();
    if spec.tab_stop {
        style = style.tab_stop();
    }
    let window = Window::create(
        class,
        Some(parent_hwnd),
        style,
        WindowExStyle::new(),
        spec.bounds,
        "",
        handler,
    )
    .map_err(|_| BackendError::CreateFailed("node"))?;
    Ok((window.hwnd(), Some(window)))
}
