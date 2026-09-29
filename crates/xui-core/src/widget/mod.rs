#![forbid(unsafe_code)]

//! Portable painted widgets built on the backend contract.
//!
//! A widget is a thin value that owns one node through [`Control`], registers a
//! painter that draws from semantic theme tokens, and maps the node's events to
//! the app's `Msg`. Because it only uses [`Ui`](crate::Ui)'s portable
//! operations, the same widget runs on every backend — painted here, and
//! eventually delegated to a native control where the backend offers one.

mod button;
mod checkbox;
mod colorpanel;
mod colorpicker;
mod combobox;
mod control;
mod dialog;
mod edit;
mod ellipsis;
mod flow_text;
mod gridview;
mod groupbox;
mod hyperlink;
mod icon;
mod label;
mod listview;
pub(crate) mod lucide;
mod materialstatusbar;
mod menu;
mod multilineedit;
mod numberfield;
mod panel;
mod placeable;
mod popup;
mod progressbar;
mod radiogroup;
pub mod scrollbar;
mod scrollview;
mod separator;
mod slider;
mod split;
mod statusbar;
mod tabs;
mod task_dialog;
mod togglebutton;
mod toolbar;
mod tooltip;
mod topbar;
mod treeview;

#[cfg(test)]
mod tests;

pub use button::Button;
pub use checkbox::CheckBox;
pub use colorpanel::{BASIC_COLORS, ColorField, ColorPanel, Hsv, HueSlider};
pub use colorpicker::ColorPicker;
pub use combobox::ComboBox;
pub use control::{Control, HasText};
pub use dialog::{Dialog, DialogAction};
pub use edit::Edit;
pub use flow_text::{FlowText, Run, RunStyle};
pub use gridview::{GridModel, GridView, Tile, TilePaint, TileSize};
pub use groupbox::GroupBox;
pub use hyperlink::Hyperlink;
pub use icon::Icon;
pub use label::Label;
pub use listview::{
    CellData, Column, ColumnWidth, Fill, ListModel, ListView, SelectionMode, SortDirection,
};
pub use lucide::Lucide;
pub use materialstatusbar::MaterialStatusBar;
pub use menu::{Menu, MenuId, MenuScope};
pub use multilineedit::MultilineEdit;
pub use numberfield::NumberField;
pub use panel::Panel;
pub use placeable::Placeable;
pub use progressbar::ProgressBar;
pub use radiogroup::RadioGroup;
pub use scrollbar::ScrollBar;
pub use scrollview::ScrollView;
pub use separator::{Orientation, Separator};
pub use slider::Slider;
pub use split::Split;
pub use statusbar::StatusBar;
pub use tabs::Tabs;
pub use task_dialog::{TaskDialog, TaskDialogAction, TaskDialogIcon};
pub use togglebutton::ToggleButton;
pub use toolbar::Toolbar;
pub use tooltip::Tooltip;
pub(crate) use topbar::draw_glyph;
pub use topbar::{Glyph, TopBar, TopBarId};
pub use treeview::{CheckState, NodeId, RowIcon, TreeModel, TreeNode, TreeRow, TreeView};
