#![forbid(unsafe_code)]

//! Tables in the flow: the grid's geometry, hit testing and vertical motion
//! through cells, rows kept whole on pages with the header repeated, and
//! incremental and viewport-first layout agreeing with a full one.

use xui_core::geometry::Point;

use super::*;
use crate::layout::{Pages, TableLayout};
use crate::model::{
    CellMark, CellStart, DocPos, EditOp, ObjectTable, ParaStyleId, Paragraph, StyleTable, Table,
    TableTable,
};

const SENTENCE: &str = "the quick brown fox jumps over the lazy dog and keeps running far away";

/// "before", a table of `rows` (each cell's text), and "after".
fn table_doc(rows: &[&[&str]], header: bool) -> Document {
    let mut tables = TableTable::new();
    let mut table = Table::new(rows[0].len());
    table.header = header;
    let id = tables.insert(table);
    let para = |text: &str| Paragraph::new(text, ParaStyleId::DEFAULT, CharStyleId::DEFAULT);
    let mut paras = vec![para("before")];
    for row in rows {
        for (col, text) in row.iter().enumerate() {
            let start = if col == 0 {
                CellStart::Row
            } else {
                CellStart::Cell
            };
            paras.push(para(text).with_cell(Some(CellMark { table: id, start })));
        }
    }
    paras.push(para("after"));
    Document::from_parts_with_tables(paras, StyleTable::new(), ObjectTable::new(), tables).unwrap()
}

fn grid() -> Document {
    table_doc(&[&["a", "b", "c"], &["d", "e", "f"]], false)
}

fn table(layout: &Layout) -> &TableLayout {
    &layout.tables()[0]
}

/// Tops and heights.
type Spans = Vec<(f32, f32)>;

/// Paragraph tops, heights and table rows, for comparing two layouts.
fn geometry(layout: &Layout) -> (Spans, Spans, Vec<f32>) {
    let paras = layout
        .paragraphs()
        .iter()
        .map(|p| (p.y, p.height))
        .collect();
    let rows = layout
        .tables()
        .iter()
        .flat_map(|t| t.rows.iter().map(|r| (r.top, r.height)))
        .collect();
    let repeats = layout
        .tables()
        .iter()
        .flat_map(|t| t.repeats.clone())
        .collect();
    (paras, rows, repeats)
}

#[test]
fn cells_sit_side_by_side_in_their_columns() {
    let doc = grid();
    let layout = lay(&doc, 300.0);
    let t = table(&layout);
    assert_eq!(t.edges, [0.0, 100.0, 200.0, 300.0]);
    // One 18 px line and 4 px of padding above and below.
    assert_eq!(t.rows[0].top, 18.0);
    assert_eq!(t.rows[0].height, 26.0);
    assert_eq!(t.rows[1].top, 44.0);
    let b = &layout.paragraphs()[2];
    assert_eq!(b.y, 22.0);
    assert_eq!(b.lines[0].x, 104.0);
    assert_eq!(layout.paragraphs()[7].y, t.bottom());
    assert_eq!(layout.height(), 70.0 + 18.0);
}

#[test]
fn a_row_is_as_tall_as_its_tallest_cell() {
    let doc = table_doc(&[&["short", SENTENCE, "x"]], false);
    let layout = lay(&doc, 300.0);
    let t = table(&layout);
    let lines = layout.paragraphs()[2].lines.len();
    assert!(lines > 1);
    assert_eq!(t.rows[0].height, lines as f32 * 18.0 + 8.0);
    for line in &layout.paragraphs()[2].lines {
        let right = line.items.last().map_or(0.0, |i| i.x + i.width);
        assert!(right <= 196.0 + 7.0, "a line runs to {right}");
    }
}

#[test]
fn clicks_land_in_the_cell_under_the_pointer() {
    let doc = grid();
    let layout = lay(&doc, 300.0);
    // Row 1, column 1 ("e"), right of its text.
    assert_eq!(layout.pos_at(&doc, Point::new(180, 55)), DocPos::new(5, 1));
    assert_eq!(layout.pos_at(&doc, Point::new(5, 30)), DocPos::new(1, 0));
    assert_eq!(layout.pos_at(&doc, Point::new(290, 30)), DocPos::new(3, 1));
    let caret = layout.caret_rect(&doc, DocPos::new(6, 0));
    assert_eq!((caret.left, caret.top), (204, 48));
}

#[test]
fn up_and_down_move_through_columns() {
    let doc = grid();
    let layout = lay(&doc, 300.0);
    // Down from "before" at x 150 enters the middle column.
    let (pos, x) = layout.vertical(DocPos::new(0, 0), Some(150.0), 1);
    assert_eq!(pos.para, 2);
    let (pos, x) = layout.vertical(pos, Some(x), 1);
    assert_eq!(pos.para, 5);
    let (pos, _) = layout.vertical(pos, Some(x), 1);
    assert_eq!(pos.para, 7);
    let (pos, _) = layout.vertical(DocPos::new(7, 0), Some(250.0), -1);
    assert_eq!(pos.para, 6);
    let (pos, _) = layout.vertical(pos, Some(250.0), -2);
    assert_eq!(pos.para, 0);
}

#[test]
fn a_row_that_would_cross_a_page_starts_the_next_one_under_the_header() {
    let rows: Vec<[&str; 2]> = vec![["h", "h"]; 6];
    let rows: Vec<&[&str]> = rows.iter().map(|r| &r[..]).collect();
    let doc = table_doc(&rows, true);
    let mut layout = Layout::new();
    layout.set_metrics(300.0, 96);
    layout.set_pages(Pages::new(100.0, 150.0));
    layout.update(&doc, &Mono);
    let t = table(&layout);
    let tops: Vec<f32> = t.rows.iter().map(|r| r.top).collect();
    // 18 + 3 x 26 = 96: the fourth row would cross 100, so it moves to the
    // next page, below the repeated header; so does the sixth, past 250.
    assert_eq!(tops, [18.0, 44.0, 70.0, 176.0, 202.0, 326.0]);
    assert_eq!(t.repeats, [150.0, 300.0]);
    let pages = layout.pages().unwrap();
    for row in &t.rows {
        assert!(pages.fits(row.top, row.height), "{row:?}");
    }
}

#[test]
fn a_row_taller_than_a_page_is_split_between_lines() {
    let long = format!("{SENTENCE} {SENTENCE} {SENTENCE} {SENTENCE}");
    let doc = table_doc(&[&[&long, "x"]], false);
    let mut layout = Layout::new();
    layout.set_metrics(120.0, 96);
    layout.set_pages(Pages::new(100.0, 150.0));
    layout.update(&doc, &Mono);
    let pages = layout.pages().unwrap();
    let cell = &layout.paragraphs()[1];
    assert!(layout.page_count() > 2);
    for line in &cell.lines {
        assert!(pages.fits(cell.y + line.y, line.height));
    }
}

#[test]
fn editing_a_cell_relays_out_to_the_same_result() {
    let mut doc = table_doc(&[&["a", "b"], &["c", "d"]], false);
    let mut layout = lay(&doc, 200.0);
    doc.apply(EditOp::InsertText {
        at: DocPos::new(2, 1),
        text: format!(" {SENTENCE}"),
        style: None,
    })
    .unwrap();
    layout.mark_dirty(2);
    layout.update(&doc, &Mono);
    assert_eq!(geometry(&layout), geometry(&lay(&doc, 200.0)));
    // A split inside a cell changes the paragraph count.
    doc.apply(EditOp::SplitParagraph {
        at: DocPos::new(2, 3),
    })
    .unwrap();
    layout.splice(2, 1, 2);
    layout.update(&doc, &Mono);
    assert_eq!(geometry(&layout), geometry(&lay(&doc, 200.0)));
    assert_eq!(table(&layout).rows[0].cells[1], 2..4);
}

#[test]
fn viewport_first_layout_finishes_like_a_full_one() {
    let rows: Vec<[&str; 3]> = (0..60).map(|_| ["a", SENTENCE, "c"]).collect();
    let rows: Vec<&[&str]> = rows.iter().map(|r| &r[..]).collect();
    let doc = table_doc(&rows, true);
    for pages in [None, Pages::new(300.0, 340.0)] {
        let mut full = Layout::new();
        full.set_metrics(300.0, 96);
        full.set_pages(pages);
        full.update(&doc, &Mono);

        let mut layout = Layout::new();
        layout.set_metrics(300.0, 96);
        layout.set_pages(pages);
        layout.update_around(&doc, &Mono, 2000.0, 2600.0, 8);
        while layout.update_idle(&doc, &Mono, 4) {}
        assert_eq!(geometry(&layout), geometry(&full));
    }
}

#[test]
fn a_visible_span_covers_whole_rows() {
    let doc = grid();
    let layout = lay(&doc, 300.0);
    // A span inside the second row only.
    let range = layout.visible(50.0, 52.0);
    assert!(range.start <= 4 && range.end >= 7, "{range:?}");
}
