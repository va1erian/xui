#![forbid(unsafe_code)]

//! The table row: inserting a table, adding and deleting rows and columns,
//! and the header and border switches, enabled while the caret is in a table.

use std::rc::Rc;

use xui_core::Dip;
use xui_core::app::Ui;
use xui_core::arrange::{Layout, LayoutExt, row, spacer};
use xui_core::backend::Result;
use xui_core::layout::Insets;
use xui_core::widget::{Button, Lucide, ToggleButton, Tooltip};
use xui_rich_text::edit::{Command, TableCursor};

use crate::app::Msg;

/// A table action from the table row.
#[derive(Clone, Copy, Debug)]
pub enum TableAction {
    /// A 3 x 3 table at the caret.
    Insert,
    RowAbove,
    RowBelow,
    ColumnLeft,
    ColumnRight,
    DeleteRows,
    DeleteColumns,
    DeleteTable,
    Header(bool),
    Border(bool),
}

impl TableAction {
    /// The editor command for this action; the switches need the caret's
    /// table.
    pub fn command(self, cursor: Option<TableCursor>) -> Option<Command> {
        Some(match self {
            TableAction::Insert => Command::InsertTable {
                rows: 3,
                columns: 3,
            },
            TableAction::RowAbove => Command::InsertRow { below: false },
            TableAction::RowBelow => Command::InsertRow { below: true },
            TableAction::ColumnLeft => Command::InsertColumn { right: false },
            TableAction::ColumnRight => Command::InsertColumn { right: true },
            TableAction::DeleteRows => Command::DeleteRows,
            TableAction::DeleteColumns => Command::DeleteColumns,
            TableAction::DeleteTable => Command::DeleteTable,
            TableAction::Header(on) | TableAction::Border(on) => {
                let cursor = cursor?;
                let mut table = cursor.table;
                if matches!(self, TableAction::Header(_)) {
                    table.header = on;
                } else {
                    table.border = on;
                }
                Command::SetTable {
                    id: cursor.id,
                    table,
                }
            }
        })
    }
}

/// The table row's controls.
pub struct TableTools {
    insert: Rc<Button<Msg>>,
    /// The buttons that act on the caret's table.
    edits: Vec<Rc<Button<Msg>>>,
    header: Rc<ToggleButton<Msg>>,
    border: Rc<ToggleButton<Msg>>,
    _tips: Vec<Tooltip<Msg>>,
}

/// The width of an icon-only button.
const ICON_WIDTH: Dip = Dip(32.0);

impl TableTools {
    /// Builds the controls.
    pub fn new(ui: &Ui<Msg>) -> Result<TableTools> {
        let mut tips = Vec::new();
        let mut button = |icon: Option<Lucide>, text: &str, tip: &str, action: TableAction| {
            let label = if icon.is_some() { "" } else { text };
            let mut button = Button::auto(ui, label)?.on_click(move || Some(Msg::Table(action)));
            if let Some(icon) = icon {
                button = button.icon(icon);
            }
            tips.push(Tooltip::attach(ui, button.id(), tip)?);
            Result::Ok(Rc::new(button))
        };
        let insert = button(Some(Lucide::Table), "", "Insert table", TableAction::Insert)?;
        let edits = vec![
            button(
                Some(Lucide::BetweenHorizontalStart),
                "",
                "Insert row above",
                TableAction::RowAbove,
            )?,
            button(
                Some(Lucide::BetweenHorizontalEnd),
                "",
                "Insert row below",
                TableAction::RowBelow,
            )?,
            button(
                Some(Lucide::BetweenVerticalStart),
                "",
                "Insert column left",
                TableAction::ColumnLeft,
            )?,
            button(
                Some(Lucide::BetweenVerticalEnd),
                "",
                "Insert column right",
                TableAction::ColumnRight,
            )?,
            button(None, "Delete row", "Delete rows", TableAction::DeleteRows)?,
            button(
                None,
                "Delete column",
                "Delete columns",
                TableAction::DeleteColumns,
            )?,
            button(
                Some(Lucide::Trash2),
                "",
                "Delete table",
                TableAction::DeleteTable,
            )?,
        ];
        let header = ToggleButton::auto(ui, "Header row")?
            .on_toggle(|on| Some(Msg::Table(TableAction::Header(on))));
        let border = ToggleButton::auto(ui, "Borders")?
            .on_toggle(|on| Some(Msg::Table(TableAction::Border(on))));
        let tools = TableTools {
            insert,
            edits,
            header: Rc::new(header),
            border: Rc::new(border),
            _tips: tips,
        };
        tools.sync(None);
        Ok(tools)
    }

    /// The row of controls.
    pub fn row(&self) -> Layout<Msg> {
        let mut layout = row()
            .spacing(Dip(4.0))
            .margins(Insets::symmetric(Dip(8.0), Dip(3.0)))
            .child((&self.insert).width(ICON_WIDTH))
            .child(spacer().width(Dip(6.0)));
        for (i, button) in self.edits.iter().enumerate() {
            let width = if i == 4 || i == 5 {
                Dip(104.0)
            } else {
                ICON_WIDTH
            };
            layout = layout.child(button.width(width));
        }
        layout
            .child(spacer().width(Dip(6.0)))
            .child((&self.header).width(Dip(96.0)))
            .child((&self.border).width(Dip(80.0)))
            .child(spacer())
    }

    /// Enables the controls that need a table when the caret is in one, and
    /// shows its switches.
    pub fn sync(&self, cursor: Option<&TableCursor>) {
        self.insert.set_enabled(cursor.is_none());
        for button in &self.edits {
            button.set_enabled(cursor.is_some());
        }
        for (toggle, on) in [
            (&self.header, cursor.is_some_and(|c| c.table.header)),
            (&self.border, cursor.is_some_and(|c| c.table.border)),
        ] {
            toggle.set_enabled(cursor.is_some());
            toggle.set_checked(on);
        }
    }
}
