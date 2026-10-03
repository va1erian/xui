//! Tables in the widget: Tab between cells and dragging a column edge.

mod common;
mod rig;

use rig::*;
use xui_core::message::Key;
use xui_rich_text::DocPos;
use xui_rich_text::model::{
    CellMark, CellStart, CharStyleId, Document, ObjectTable, ParaStyleId, Paragraph, Selection,
    StyleTable, Table, TableTable,
};

/// "before", a 2 x 2 table of a, b, c, d, and "after".
fn table_doc() -> Document {
    let mut tables = TableTable::new();
    let id = tables.insert(Table::new(2));
    let para = |text: &str| Paragraph::new(text, ParaStyleId::DEFAULT, CharStyleId::DEFAULT);
    let cell = |text: &str, start| para(text).with_cell(Some(CellMark { table: id, start }));
    let paras = vec![
        para("before"),
        cell("a", CellStart::Row),
        cell("b", CellStart::Cell),
        cell("c", CellStart::Row),
        cell("d", CellStart::Cell),
        para("after"),
    ];
    Document::from_parts_with_tables(paras, StyleTable::new(), ObjectTable::new(), tables).unwrap()
}

fn first_width(rig: &Rig) -> f32 {
    rig.editor.with_document(|d| {
        let id = d.table_spans()[0].id;
        d.tables().get(id).unwrap().columns[0]
    })
}

#[test]
fn tab_selects_the_next_cell() {
    run(table_doc(), |stage, rig| {
        let a = rig.editor.caret_rect(DocPos::new(1, 0), Default::default());
        mouse(stage, "down", a.left, (a.top + a.bottom) / 2);
        mouse(stage, "up", a.left, (a.top + a.bottom) / 2);
        assert_eq!(rig.editor.selection(), Selection::caret(DocPos::new(1, 0)));
        key(stage, Key::TAB);
        assert_eq!(
            rig.editor.selection(),
            Selection::text(DocPos::new(2, 0), DocPos::new(2, 1))
        );
        let cursor = rig.editor.table_cursor().unwrap();
        assert_eq!((cursor.row, cursor.column), (0, 1));
    });
}

#[test]
fn dragging_a_column_edge_resizes_both_columns_in_one_step() {
    run(table_doc(), |stage, rig| {
        assert!((first_width(rig) - 0.5).abs() < 1e-4);
        // The edge is a cell padding left of the second cell's text.
        let b = rig.editor.caret_rect(DocPos::new(2, 0), Default::default());
        let (x, y) = (b.left - 4, (b.top + b.bottom) / 2);
        mouse(stage, "down", x, y);
        mouse(stage, "move", x + 30, y);
        mouse(stage, "move", x + 60, y);
        assert!(
            (first_width(rig) - 0.5).abs() < 1e-4,
            "committed on release"
        );
        mouse(stage, "up", x + 60, y);
        let wider = first_width(rig);
        assert!(wider > 0.6, "{wider}");
        ctrl(stage, Key::Z);
        assert!((first_width(rig) - 0.5).abs() < 1e-4, "one undo step");
    });
}
