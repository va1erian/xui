//! Tables: inserting them, editing inside cells, rows and columns, deleting
//! across cells, copy and paste, and undo of all of it.

#[path = "edit_common.rs"]
mod harness;

use harness::{Ed, undo};
use xui_rich_text::DocPos;
use xui_rich_text::edit::{Command, Motion};
use xui_rich_text::model::{CellStart, Document, Selection, Table, TableSpan};

/// The only table's grid.
fn grid(ed: &Ed) -> TableSpan {
    let spans = ed.state.doc.table_spans();
    assert_eq!(spans.len(), 1, "expected one table");
    spans.into_iter().next().unwrap()
}

/// Each cell's text, row by row, a cell's paragraphs joined by `/`.
fn cells(ed: &Ed) -> Vec<Vec<String>> {
    let doc = &ed.state.doc;
    grid(ed)
        .rows
        .iter()
        .map(|row| {
            row.iter()
                .map(|cell| {
                    doc.paragraphs()[cell.clone()]
                        .iter()
                        .map(|p| p.text())
                        .collect::<Vec<_>>()
                        .join("/")
                })
                .collect()
        })
        .collect()
}

/// An editor on "before|after" with a 2 x 3 table inserted at the bar.
fn with_table() -> Ed {
    let mut ed = Ed::new("beforeafter");
    ed.caret_at(0, 6);
    ed.run(Command::InsertTable {
        rows: 2,
        columns: 3,
    });
    ed
}

/// Types into each cell in reading order with Tab between them.
fn fill(ed: &mut Ed, texts: &[&str]) {
    let first = grid(ed).paras.start;
    ed.caret_at(first, 0);
    for (i, text) in texts.iter().enumerate() {
        if i > 0 {
            ed.run(Command::NextCell);
        }
        ed.type_str(text);
    }
}

#[test]
fn inserting_a_table_splits_the_paragraph_and_enters_the_first_cell() {
    let mut ed = with_table();
    let span = grid(&ed);
    assert_eq!(span.paras, 1..7);
    assert_eq!(span.rows.len(), 2);
    assert_eq!(span.columns(), 3);
    assert_eq!(ed.state.doc.paragraphs()[0].text(), "before");
    assert_eq!(ed.state.doc.paragraphs()[7].text(), "after");
    assert_eq!(ed.caret(), DocPos::new(1, 0));
    let cursor = ed.state.table_cursor().unwrap();
    assert_eq!((cursor.row, cursor.column, cursor.rows), (0, 0, 2));
    assert!(cursor.table.border && !cursor.table.header);
    undo(&mut ed);
    assert_eq!(ed.text(), "beforeafter");
    assert!(ed.state.doc.table_spans().is_empty());
    assert!(ed.state.doc.tables().is_empty());
}

#[test]
fn a_table_at_the_start_of_a_paragraph_goes_before_it() {
    let mut ed = Ed::new("text");
    ed.run(Command::InsertTable {
        rows: 1,
        columns: 2,
    });
    assert_eq!(grid(&ed).paras, 0..2);
    assert_eq!(ed.state.doc.paragraphs()[2].text(), "text");
    // No table inside a table.
    let before = ed.state.doc.paragraph_count();
    assert!(
        ed.run(Command::InsertTable {
            rows: 1,
            columns: 1
        })
        .is_none()
    );
    assert_eq!(ed.state.doc.paragraph_count(), before);
}

#[test]
fn typing_enter_and_backspace_stay_inside_a_cell() {
    let mut ed = with_table();
    ed.type_str("ab");
    ed.run(Command::InsertParagraph);
    ed.type_str("cd");
    assert_eq!(cells(&ed)[0][0], "ab/cd");
    // Backspace at the start of the next cell steps back without joining.
    let next = grid(&ed).rows[0][1].start;
    ed.caret_at(next, 0);
    ed.run(Command::Backspace);
    assert_eq!(ed.caret(), DocPos::new(next - 1, 2));
    assert_eq!(cells(&ed)[0][0], "ab/cd");
    // Delete at a cell's end does nothing.
    assert!(ed.run(Command::Delete).is_none());
    assert_eq!(grid(&ed).rows[0].len(), 3);
    // Backspace inside the cell joins its own paragraphs.
    ed.caret_at(2, 0);
    ed.run(Command::Backspace);
    assert_eq!(cells(&ed)[0][0], "abcd");
}

#[test]
fn tab_selects_the_next_cell_and_adds_a_row_after_the_last() {
    let mut ed = with_table();
    fill(&mut ed, &["a", "b", "c", "d", "e", "f"]);
    assert_eq!(cells(&ed), [["a", "b", "c"], ["d", "e", "f"]]);
    ed.run(Command::PrevCell);
    let e = grid(&ed).rows[1][1].start;
    assert_eq!(
        ed.state.selection,
        Selection::text(DocPos::new(e, 0), DocPos::new(e, 1))
    );
    ed.run(Command::NextCell);
    ed.run(Command::NextCell);
    assert_eq!(grid(&ed).rows.len(), 3);
    assert_eq!(ed.state.table_cursor().unwrap().row, 2);
    // Shift+Tab in the first cell stays.
    ed.caret_at(grid(&ed).paras.start, 0);
    assert!(ed.run(Command::PrevCell).is_none());
}

#[test]
fn rows_go_in_above_and_below_and_come_out() {
    let mut ed = with_table();
    fill(&mut ed, &["a", "b", "c", "d", "e", "f"]);
    let d = grid(&ed).rows[1][0].start;
    ed.caret_at(d, 0);
    ed.run(Command::InsertRow { below: false });
    assert_eq!(cells(&ed), [["a", "b", "c"], ["", "", ""], ["d", "e", "f"]]);
    assert_eq!(ed.state.table_cursor().unwrap().row, 1);
    ed.run(Command::InsertRow { below: true });
    assert_eq!(cells(&ed).len(), 4);
    assert_eq!(ed.state.table_cursor().unwrap().row, 2);
    // Delete the rows a selection covers: the two empty ones.
    let span = grid(&ed);
    ed.select((span.rows[1][2].start, 0), (span.rows[2][0].start, 0));
    ed.run(Command::DeleteRows);
    assert_eq!(cells(&ed), [["a", "b", "c"], ["d", "e", "f"]]);
    assert_eq!(ed.state.table_cursor().unwrap().row, 1);
    undo(&mut ed);
    assert_eq!(cells(&ed).len(), 4);
}

#[test]
fn columns_go_in_left_and_right_and_come_out() {
    let mut ed = with_table();
    fill(&mut ed, &["a", "b", "c", "d", "e", "f"]);
    let a = grid(&ed).paras.start;
    ed.caret_at(a, 0);
    ed.run(Command::InsertColumn { right: false });
    assert_eq!(cells(&ed), [["", "a", "b", "c"], ["", "d", "e", "f"]]);
    let first = grid(&ed).rows[1][0].start;
    assert_eq!(
        ed.state.doc.paragraphs()[first].cell().unwrap().start,
        CellStart::Row
    );
    let cursor = ed.state.table_cursor().unwrap();
    assert_eq!((cursor.row, cursor.column), (0, 0));
    let widths = &cursor.table.columns;
    assert_eq!(widths.len(), 4);
    assert!(widths.iter().all(|w| (w - 0.25).abs() < 1e-4));

    ed.run(Command::InsertColumn { right: true });
    assert_eq!(cells(&ed)[1], ["", "", "d", "e", "f"]);
    // The first two columns, both empty, go.
    ed.select((grid(&ed).paras.start, 0), (grid(&ed).rows[1][1].start, 0));
    ed.run(Command::DeleteColumns);
    assert_eq!(cells(&ed), [["a", "b", "c"], ["d", "e", "f"]]);
    let first = grid(&ed).rows[1][0].start;
    assert_eq!(
        ed.state.doc.paragraphs()[first].cell().unwrap().start,
        CellStart::Row
    );
    let cursor = ed.state.table_cursor().unwrap();
    assert!((cursor.table.columns.iter().sum::<f32>() - 1.0).abs() < 1e-4);
    undo(&mut ed);
    assert_eq!(cells(&ed)[0].len(), 5);
}

#[test]
fn deleting_every_row_or_column_deletes_the_table() {
    let mut ed = with_table();
    let span = grid(&ed);
    ed.select((span.paras.start, 0), (span.paras.end - 1, 0));
    ed.run(Command::DeleteRows);
    assert!(ed.state.doc.table_spans().is_empty());
    assert_eq!(ed.text(), "before\nafter");
    assert_eq!(ed.caret(), DocPos::new(1, 0));
    undo(&mut ed);
    assert_eq!(grid(&ed).rows.len(), 2);

    ed.caret_at(grid(&ed).paras.start, 0);
    ed.run(Command::DeleteTable);
    assert!(ed.state.doc.tables().is_empty());
}

#[test]
fn a_selection_across_cells_clears_them_and_keeps_the_grid() {
    let mut ed = with_table();
    fill(&mut ed, &["abc", "def", "ghi", "jkl", "mno", "pqr"]);
    let span = grid(&ed);
    ed.select((span.rows[0][1].start, 1), (span.rows[1][0].start, 2));
    ed.run(Command::InsertText("X".into()));
    assert_eq!(
        cells(&ed),
        [["abc", "dX", "",], ["l", "mno", "pqr"]].map(|r| r.to_vec())
    );
    undo(&mut ed);
    assert_eq!(cells(&ed), [["abc", "def", "ghi"], ["jkl", "mno", "pqr"]]);

    // From outside into the table: the text outside goes, the cells clear.
    ed.select((0, 3), (span.rows[0][1].start, 1));
    ed.run(Command::Delete);
    assert_eq!(ed.state.doc.paragraphs()[0].text(), "bef");
    assert_eq!(cells(&ed)[0], ["", "ef", "ghi"]);
}

#[test]
fn a_selection_around_a_whole_table_deletes_it_and_undo_brings_it_back() {
    let mut ed = with_table();
    fill(&mut ed, &["a", "b", "c", "d", "e", "f"]);
    let before = ed.state.doc.clone();
    ed.select((0, 3), (7, 2));
    ed.run(Command::Delete);
    assert_eq!(ed.text(), "befter");
    assert!(ed.state.doc.tables().is_empty());
    undo(&mut ed);
    assert!(ed.state.doc.content_eq(&before));
    ed.run(Command::Redo);
    assert_eq!(ed.text(), "befter");
}

#[test]
fn a_copied_table_pastes_as_a_table_outside_and_as_text_inside_one() {
    let mut ed = with_table();
    fill(&mut ed, &["a", "b", "c", "d", "e", "f"]);
    ed.select((0, 6), (7, 0));
    ed.run(Command::Copy);
    ed.caret_at(7, 5);
    ed.run(Command::Paste);
    let spans = ed.state.doc.table_spans();
    assert_eq!(spans.len(), 2);
    assert_ne!(spans[0].id, spans[1].id);
    assert_eq!(ed.state.doc.tables().len(), 2);
    // Pasted into a cell, the copy becomes plain paragraphs of that cell.
    let cell = spans[0].rows[1][2].clone();
    ed.caret_at(cell.start, 1);
    ed.run(Command::Paste);
    assert_eq!(ed.state.doc.table_spans().len(), 2);
    assert_eq!(ed.state.doc.tables().len(), 2);
    let span = ed.state.doc.table_at(cell.start).unwrap();
    assert!(span.rows[1][2].len() > 1);
}

#[test]
fn table_settings_change_and_undo() {
    let mut ed = with_table();
    let cursor = ed.state.table_cursor().unwrap();
    let mut table = cursor.table;
    table.header = true;
    table.border = false;
    let table = table.with_widths(&[2.0, 1.0, 1.0]);
    let id = cursor.id;
    ed.run(Command::SetTable {
        id,
        table: table.clone(),
    });
    assert_eq!(ed.state.table_cursor().unwrap().table, table);
    // A different column count is refused.
    let two = Table::new(2);
    assert!(ed.run(Command::SetTable { id, table: two }).is_none());
    undo(&mut ed);
    assert!(ed.state.table_cursor().unwrap().table.border);
}

#[test]
fn plain_text_separates_cells_with_tabs() {
    let mut ed = with_table();
    fill(&mut ed, &["a", "b", "c", "d", "e", "f"]);
    assert_eq!(ed.text(), "before\na\tb\tc\nd\te\tf\nafter");
}

#[test]
fn arrows_walk_through_cells_in_reading_order() {
    let mut ed = with_table();
    fill(&mut ed, &["a", "b", "c", "d", "e", "f"]);
    ed.caret_at(1, 1);
    ed.mv(Motion::Right);
    assert_eq!(ed.caret(), DocPos::new(2, 0));
    ed.caret_at(7, 0);
    ed.mv(Motion::Left);
    assert_eq!(ed.caret(), DocPos::new(6, 1));
}

#[test]
fn undo_of_every_table_command_restores_the_document() {
    let commands = [
        Command::InsertRow { below: true },
        Command::InsertRow { below: false },
        Command::InsertColumn { right: true },
        Command::InsertColumn { right: false },
        Command::DeleteRows,
        Command::DeleteColumns,
        Command::DeleteTable,
    ];
    for command in commands {
        let mut ed = with_table();
        fill(&mut ed, &["a", "b", "c", "d", "e", "f"]);
        let span = grid(&ed);
        ed.caret_at(span.rows[1][1].start, 0);
        let before: Document = ed.state.doc.clone();
        assert!(ed.run(command.clone()).doc_changed(), "{command:?}");
        undo(&mut ed);
        assert!(ed.state.doc.content_eq(&before), "{command:?}");
        ed.run(Command::Redo);
        undo(&mut ed);
        assert!(ed.state.doc.content_eq(&before), "{command:?}");
    }
}
