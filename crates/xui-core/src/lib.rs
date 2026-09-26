#![forbid(unsafe_code)]

//! The backend-agnostic core of xui.
//!
//! Everything a widget or a backend needs that is not tied to a platform lives
//! here: pixel geometry, typed length units, colour, the pure layout
//! arithmetic, the semantic theme tokens, the input vocabulary and the
//! accessibility tree model. The crate has no platform dependency and no
//! `unsafe`, so every backend can share these types.
//!
//! [`backend`] holds the contract a backend implements (the [`Backend`] trait,
//! [`WidgetId`]/[`WindowId`], [`Event`], [`Canvas`]), and [`router`] routes a
//! backend's events to the widget that owns a node.

pub mod accessibility;
pub mod app;
pub mod backend;
pub mod color;
pub mod geometry;
pub mod image;
pub mod layout;
pub mod message;
pub mod property;
pub mod router;
pub mod theme;
pub mod units;
pub mod widget;

pub use app::{App, Proxy, Ui, run_app};

pub use backend::{
    Backend, BackendError, Canvas, Event, ImplKind, NodeKind, NodeOptions, NodeSpec, Painter,
    ParentRef, TextMetrics, TextStyle, TextVAlign, TimerId, WidgetId, WindowId,
};
pub use color::Color;
pub use geometry::{Point, Rect, Size};
pub use image::{Image, ImageError};
pub use layout::{Anchor, Dock, DockLayout, Insets, Stack, StackDirection, StackSlot, anchored};
pub use message::{HitTest, Key, Modifiers, MouseButton};
pub use property::{Properties, Property, Value};
pub use router::{Router, WidgetHost};
pub use theme::{Theme, Themed};
pub use units::{Dip, Px, dip};
pub use widget::{
    Button, CellData, CheckBox, Column, ColumnWidth, ComboBox, Control, Edit, Fill, Glyph,
    GroupBox, HasText, Hyperlink, Label, ListModel, ListView, MaterialStatusBar, MultilineEdit,
    NumberField, Panel, ProgressBar, RadioGroup, SelectionMode, Separator, Slider, SortDirection,
    StatusBar, ToggleButton, Toolbar, TopBar, TopBarId, TreeRow, TreeView,
};

/// The core types a frontend or a backend usually needs, in one `use`.
pub mod prelude {
    pub use crate::color::prelude::*;
    pub use crate::geometry::prelude::*;
    pub use crate::layout::prelude::*;
    pub use crate::message::{HitTest, Key, Modifiers, MouseButton};
    pub use crate::theme::{Theme, Themed};
    pub use crate::units::prelude::*;
}
