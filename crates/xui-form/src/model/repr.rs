#![forbid(unsafe_code)]

//! The serde form of [`Node`]: a mirror of the enum the derive is written
//! on, so that `Node` itself implements `Serialize` and `Deserialize` by hand
//! (reading counts its levels, see [`Node`]) without gaining the derive's
//! public `serialize` and `deserialize` functions.

use serde::{Deserialize, Serialize};

use super::{
    Absolute, Button, CheckBox, ComboBox, Edit, Grid, Group, Hyperlink, Label, ListView,
    MultilineEdit, Node, NumberField, Panel, ProgressBar, RadioGroup, Separator, Slider, Stack,
    Tabs, ToggleButton,
};

/// `Node`, variant for variant: the file spells each as `Kind(..)`.
#[derive(Serialize, Deserialize)]
#[serde(remote = "Node")]
pub(super) enum NodeDef {
    Row(Stack),
    Column(Stack),
    Wrap(Stack),
    Grid(Grid),
    Absolute(Absolute),
    Panel(Panel),
    Group(Group),
    Tabs(Tabs),
    Label(Label),
    Button(Button),
    Hyperlink(Hyperlink),
    CheckBox(CheckBox),
    ToggleButton(ToggleButton),
    Edit(Edit),
    MultilineEdit(MultilineEdit),
    NumberField(NumberField),
    Slider(Slider),
    ProgressBar(ProgressBar),
    RadioGroup(RadioGroup),
    ComboBox(ComboBox),
    ListView(ListView),
    Separator(Separator),
}
