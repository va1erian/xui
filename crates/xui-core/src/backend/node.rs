#![forbid(unsafe_code)]

//! What the front layer asks a backend to create: a [`NodeKind`] and its
//! [`NodeSpec`], plus how the backend implements it ([`ImplKind`]).

use crate::geometry::Rect;

use super::{WidgetId, WindowId};

/// What a new node is parented to: the window directly, or an existing node
/// (a [`Panel`](NodeKind::Panel), [`ScrollView`](NodeKind::ScrollView) or
/// [`Tabs`](NodeKind::Tabs) container).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ParentRef {
    /// A direct child of the window.
    Window(WindowId),
    /// A child of another node.
    Widget(WidgetId),
}

impl From<WindowId> for ParentRef {
    fn from(window: WindowId) -> ParentRef {
        ParentRef::Window(window)
    }
}

impl From<WidgetId> for ParentRef {
    fn from(widget: WidgetId) -> ParentRef {
        ParentRef::Widget(widget)
    }
}

/// The kind of widget a backend should create.
///
/// This is the portable vocabulary of the widget layer; the backend decides
/// whether it maps to a native control ([`ImplKind::Native`]) or a region the
/// front layer paints ([`ImplKind::Painted`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NodeKind {
    /// A plain container with no behaviour of its own.
    Container,
    /// A static text label.
    Label,
    /// A push button.
    Button,
    /// A check box.
    CheckBox,
    /// A radio option belonging to a group.
    Radio,
    /// A group box drawn around related widgets.
    GroupBox,
    /// A range progress indicator.
    ProgressBar,
    /// A draggable range control.
    Slider,
    /// A text-entry field.
    Edit,
    /// A drop-down list.
    ComboBox,
    /// A virtualized report list.
    ListView,
    /// A lazy keyed tree.
    TreeView,
    /// A tool strip.
    Toolbar,
    /// A paged tab strip.
    Tabs,
    /// A window status bar.
    StatusBar,
    /// A scrollable viewport.
    ScrollView,
    /// A scrolling panel of standard controls.
    Panel,
    /// An application-defined owner-drawn widget.
    Custom,
}

/// Per-kind creation options that do not fit the common fields.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum NodeOptions {
    /// No extra options.
    #[default]
    None,
    /// An [`Edit`](NodeKind::Edit) field's shape.
    Edit {
        /// Whether the field accepts multiple lines.
        multiline: bool,
        /// Whether the text is masked.
        password: bool,
    },
    /// A [`Button`](NodeKind::Button) that activates on Enter.
    DefaultButton,
    /// Whether arrow keys are handled by the widget rather than navigating.
    CaptureArrowKeys,
}

/// A backend-neutral description of a node to create.
#[derive(Clone, Debug, PartialEq)]
pub struct NodeSpec {
    /// What to create.
    pub kind: NodeKind,
    /// The initial bounds, in device pixels.
    pub bounds: Rect,
    /// The initial text, if the kind shows one.
    pub text: String,
    /// Extra per-kind options.
    pub options: NodeOptions,
    /// Whether the node starts visible.
    pub visible: bool,
    /// Whether the node starts enabled.
    pub enabled: bool,
    /// Whether the node takes part in the Tab order.
    pub tab_stop: bool,
}

impl NodeSpec {
    /// A visible, enabled node of `kind` at `bounds` with no text.
    pub fn new(kind: NodeKind, bounds: Rect) -> NodeSpec {
        NodeSpec {
            kind,
            bounds,
            text: String::new(),
            options: NodeOptions::None,
            visible: true,
            enabled: true,
            tab_stop: false,
        }
    }

    /// Sets the node's initial text.
    pub fn text(mut self, text: impl Into<String>) -> NodeSpec {
        self.text = text.into();
        self
    }

    /// Sets the node's extra options.
    pub fn options(mut self, options: NodeOptions) -> NodeSpec {
        self.options = options;
        self
    }

    /// Includes the node in the Tab order.
    pub fn tab_stop(mut self) -> NodeSpec {
        self.tab_stop = true;
        self
    }
}

/// How a backend implements a [`NodeKind`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImplKind {
    /// The backend owns the widget (a native control or its equivalent).
    Native,
    /// The front layer paints the widget through [`Canvas`](super::Canvas).
    Painted,
}
