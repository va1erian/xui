#![forbid(unsafe_code)]

//! The strip's items, where the app's parts were placed, and the state mirror a
//! host or test can read.

use xui_core::app::Ui;
use xui_core::backend::WidgetId;
use xui_core::geometry::Rect;

use super::toolbar::StripItem;
use crate::model::{Pixel, SIZES, Tool};

/// The tool, size and action cells, in order. Save/Open are included only when
/// the storage is available.
pub(super) fn strip_items(io: bool) -> Vec<StripItem> {
    let mut items: Vec<StripItem> = Tool::ALL.into_iter().map(StripItem::Tool).collect();
    items.extend(SIZES.into_iter().map(StripItem::Size));
    items.extend([
        StripItem::Undo,
        StripItem::Redo,
        StripItem::Clear,
        StripItem::New,
    ]);
    if io {
        items.extend([StripItem::Save, StripItem::Open]);
    }
    items
}

// xui gap: G12 — the runtime owns the app and there is no public handle to it
// after `render_with`, so a test reads this mirror instead.

/// A read-only mirror of the app's state, updated after every message.
///
/// A host embedding the library can pass one to [`PaintApp::build_observed`] to
/// drive, say, its own window title or tray, and a test uses it to read the
/// model without reaching into the widgets. Without one, the app works exactly
/// as before.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Observer {
    /// The active tool.
    pub tool: Tool,
    /// The brush diameter.
    pub size: u32,
    /// The primary colour.
    pub primary: Pixel,
    /// The secondary colour.
    pub secondary: Pixel,
    /// Whether undo is available.
    pub can_undo: bool,
    /// Whether redo is available.
    pub can_redo: bool,
    /// The cursor in canvas pixels, if it is over the canvas.
    pub cursor: Option<(i32, i32)>,
    /// Whether a drag is in progress.
    pub dragging: bool,
    /// The four status-bar parts.
    pub status: [String; 4],
    /// The app's widget nodes, set once the app is built.
    pub parts: Option<Parts>,
}

impl Default for Observer {
    fn default() -> Observer {
        Observer {
            tool: Tool::Pencil,
            size: 1,
            primary: [0, 0, 0, 255],
            secondary: [255, 255, 255, 255],
            can_undo: false,
            can_redo: false,
            cursor: None,
            dragging: false,
            status: Default::default(),
            parts: None,
        }
    }
}

/// The app's widget nodes, so a host or test can find where the layout put
/// them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Parts {
    /// The tool strip.
    pub toolbar: WidgetId,
    /// The drawing canvas.
    pub canvas: WidgetId,
    /// The colour palette.
    pub palette: WidgetId,
    /// The status bar.
    pub status: WidgetId,
}

impl Parts {
    /// Where the window's layout has placed each part, in device pixels.
    pub fn layout<M: 'static>(&self, ui: &Ui<M>) -> Layout {
        Layout {
            toolbar: ui.bounds(self.toolbar),
            canvas: ui.bounds(self.canvas),
            palette: ui.bounds(self.palette),
            status: ui.bounds(self.status),
        }
    }
}

/// The device-pixel rectangles the app's widgets are placed in: the tool
/// strip across the top, the canvas filling the middle, then the palette and
/// the status bar at the bottom.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Layout {
    /// The tool strip across the top.
    pub toolbar: Rect,
    /// The drawing canvas.
    pub canvas: Rect,
    /// The colour palette.
    pub palette: Rect,
    /// The status bar at the bottom.
    pub status: Rect,
}
