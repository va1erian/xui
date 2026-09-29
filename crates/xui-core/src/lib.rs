#![forbid(unsafe_code)]
#![warn(missing_docs)]

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
pub mod arrange;
pub mod backend;
pub mod color;
pub mod geometry;
pub mod icon;
pub mod image;
pub mod layout;
pub mod message;
pub mod property;
pub mod router;
pub mod theme;
pub mod units;
pub mod widget;

pub use app::{App, Proxy, Ui, WindowHandle, run_app};

pub use backend::{
    Backdrop, Backend, BackendError, Canvas, Cap, Corner, Dash, Decorations, Event, FontSpec,
    GradientStop, ImplKind, LinearGradient, NativeWindowHandle, NodeKind, NodeOptions, NodeSpec,
    Painter, ParentRef, PlatformSpec, RadialGradient, Rgba, Stroke, TextHit, TextLayout,
    TextMetrics, TextShaper, TextStyle, TextVAlign, TimerId, WidgetId, WindowId,
};
pub use color::Color;
pub use geometry::{Point, Rect, Size};
pub use icon::{IconRef, Lucide, draw_icon};
pub use image::{Image, ImageError};
pub use layout::{Anchor, Dock, DockLayout, Insets, Stack, StackDirection, StackSlot, anchored};
pub use message::{HitTest, Key, Modifiers, MouseButton};
pub use property::{Properties, Property, Value};
pub use router::{Router, WidgetHost};
pub use theme::{Theme, Themed};
pub use units::{Dip, Px, dip};
pub use widget::{
    BASIC_COLORS, Button, CellData, CheckBox, CheckState, ColorField, ColorPanel, ColorPicker,
    Column, ColumnWidth, ComboBox, Control, Dialog, DialogAction, Edit, Fill, FlowText, Glyph,
    GridModel, GridView, GroupBox, HasText, Hsv, HueSlider, Hyperlink, Icon, Label, ListModel,
    ListView, MaterialStatusBar, Menu, MenuId, MenuScope, MultilineEdit, NodeId, NumberField,
    Panel, ProgressBar, RadioGroup, RowIcon, Run, RunStyle, ScrollView, SelectionMode, Separator,
    Slider, SortDirection, Split, StatusBar, Tabs, Tile, TilePaint, TileSize, ToggleButton,
    Toolbar, Tooltip, TopBar, TopBarId, TreeModel, TreeNode, TreeRow, TreeView,
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
