#![forbid(unsafe_code)]

//! The table row: inserting a table, adding and deleting rows and columns,
//! and the header and border switches, enabled while the caret is in a table.

use xui_core::Dip;
use xui_core::arrange::{Entry, Handle, Layout, LayoutExt, button, row, spacer, toggle_button};
use xui_core::layout::Insets;
use xui_core::widget::{Button, Lucide, ToggleButton};
use xui_rich_text::edit::{Command, TableCursor};

use crate::app::Msg;
use crate::ui::{ICON_WIDTH, Tips, tipped};

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

/// The buttons that act on the caret's table: icon (a text button has none),
/// text, tooltip and action.
const EDITS: [(Option<Lucide>, &str, &str, TableAction); 7] = [
    (
        Some(Lucide::BetweenHorizontalStart),
        "",
        "Insert row above",
        TableAction::RowAbove,
    ),
    (
        Some(Lucide::BetweenHorizontalEnd),
        "",
        "Insert row below",
        TableAction::RowBelow,
    ),
    (
        Some(Lucide::BetweenVerticalStart),
        "",
        "Insert column left",
        TableAction::ColumnLeft,
    ),
    (
        Some(Lucide::BetweenVerticalEnd),
        "",
        "Insert column right",
        TableAction::ColumnRight,
    ),
    (None, "Delete row", "Delete rows", TableAction::DeleteRows),
    (
        None,
        "Delete column",
        "Delete columns",
        TableAction::DeleteColumns,
    ),
    (
        Some(Lucide::Trash2),
        "",
        "Delete table",
        TableAction::DeleteTable,
    ),
];

/// The table row's controls.
#[derive(Default)]
pub struct TableTools {
    insert: Handle<Button<Msg>>,
    /// The buttons that act on the caret's table, in [`EDITS`] order.
    edits: [Handle<Button<Msg>>; 7],
    header: Handle<ToggleButton<Msg>>,
    border: Handle<ToggleButton<Msg>>,
    tips: Tips,
}

impl TableTools {
    /// The row of controls, bound to these handles.
    pub fn row(&self) -> Layout<Msg> {
        let edits: Vec<Entry<Msg>> = self
            .edits
            .iter()
            .zip(EDITS)
            .map(|(handle, (icon, text, tip, action))| {
                self.action_button(handle, icon, text, tip, action)
            })
            .collect();
        row()
            .gap(4)
            .padding(Insets::symmetric(Dip(8.0), Dip(3.0)))
            .child(self.action_button(
                &self.insert,
                Some(Lucide::Table),
                "",
                "Insert table",
                TableAction::Insert,
            ))
            .child(spacer().width(6))
            .children(edits)
            .children((
                spacer().width(6),
                toggle_button("Header row")
                    .bind(&self.header)
                    .on_toggle(|on| Msg::Table(TableAction::Header(on)))
                    .width(96),
                toggle_button("Borders")
                    .bind(&self.border)
                    .on_toggle(|on| Msg::Table(TableAction::Border(on)))
                    .width(80),
                spacer(),
            ))
    }

    /// A button bound to `handle` that raises `action` and is named `tip` on
    /// hover: icon-only when it has an icon, else showing `text`.
    fn action_button(
        &self,
        handle: &Handle<Button<Msg>>,
        icon: Option<Lucide>,
        text: &str,
        tip: &'static str,
        action: TableAction,
    ) -> Entry<Msg> {
        let mut button = button(text)
            .bind(handle)
            .on_click_with(move || Some(Msg::Table(action)));
        let width = match icon {
            Some(icon) => {
                button = button.then(move |button| button.icon(icon));
                ICON_WIDTH
            }
            None => Dip(104.0),
        };
        tipped(button, &self.tips, tip).width(width)
    }

    /// Enables the controls that need a table when the caret is in one, and
    /// shows its switches.
    pub fn sync(&self, cursor: Option<&TableCursor>) {
        self.insert.get().set_enabled(cursor.is_none());
        for button in &self.edits {
            button.get().set_enabled(cursor.is_some());
        }
        for (toggle, on) in [
            (&self.header, cursor.is_some_and(|c| c.table.header)),
            (&self.border, cursor.is_some_and(|c| c.table.border)),
        ] {
            let toggle = toggle.get();
            toggle.set_enabled(cursor.is_some());
            toggle.set_checked(on);
        }
    }
}
