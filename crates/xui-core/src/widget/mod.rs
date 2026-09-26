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
mod combobox;
mod control;
mod edit;
mod groupbox;
mod hyperlink;
mod label;
mod listview;
mod progressbar;
mod radiogroup;
mod separator;
mod slider;

#[cfg(test)]
mod tests;

pub use button::Button;
pub use checkbox::CheckBox;
pub use combobox::ComboBox;
pub use control::{Control, HasText};
pub use edit::Edit;
pub use groupbox::GroupBox;
pub use hyperlink::Hyperlink;
pub use label::Label;
pub use listview::ListView;
pub use progressbar::ProgressBar;
pub use radiogroup::RadioGroup;
pub use separator::Separator;
pub use slider::Slider;
