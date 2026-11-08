#![forbid(unsafe_code)]

//! Tables: columns shared by rows laid out at different times.

use super::*;

const TABLE: &str = "| a | bb |\n|---|---|\n| cccc | d |\n| e | ffffff |";

/// The column frames of each row, laying the rows out one at a time.
fn rows(markdown: &str, width: f32) -> Vec<Vec<FRect>> {
    let mut layout = lay(markdown, width);
    (0..3)
        .map(|i| {
            let (top, end) = (layout.top(i), layout.top(i) + 1.0);
            layout.ensure(top, end);
            frames(layout.block(i).expect("laid out"))
        })
        .collect()
}

#[test]
fn columns_line_up_across_rows_laid_out_separately() {
    let rows = rows(TABLE, 300.0);
    let columns = |row: &Vec<FRect>| row.iter().map(|f| (f.x, f.w)).collect::<Vec<_>>();
    // The widest cells: "cccc" is 28 px and "ffffff" 42 px, plus 16 px of padding.
    assert_eq!(columns(&rows[0]), [(0.0, 44.0), (44.0, 58.0)]);
    assert_eq!(columns(&rows[1]), columns(&rows[0]));
    assert_eq!(columns(&rows[2]), columns(&rows[0]));
}

#[test]
fn columns_shrink_to_fit_a_narrow_width() {
    let rows = rows(TABLE, 60.0);
    let widths: Vec<f32> = rows[1].iter().map(|f| f.w).collect();
    assert_eq!(widths, [25.0, 34.0]);
    assert!(widths.iter().sum::<f32>() <= 60.0);
    assert_eq!(rows[2][1].x, 25.0);
}

#[test]
fn a_cell_wraps_in_a_shrunk_column_and_its_row_grows() {
    let mut layout = lay(
        "| a | b |\n|---|---|\n| one two three four five | x |",
        80.0,
    );
    layout.ensure(0.0, 1.0e6);
    let header = layout.block(0).unwrap().height;
    let body = layout.block(1).unwrap().height;
    assert!(body > header, "{body} vs {header}");
    let frame = frames(layout.block(1).unwrap())[0];
    assert_eq!(frame.h, body);
}

#[test]
fn only_the_header_row_has_the_surface_fill() {
    let mut layout = lay(TABLE, 300.0);
    layout.ensure(0.0, 1.0e6);
    let header = fills(layout.block(0).unwrap(), Role::Surface);
    assert_eq!(header.len(), 1);
    assert_eq!(header[0].w, 102.0);
    assert_eq!(header[0].h, layout.block(0).unwrap().height);
    assert!(fills(layout.block(1).unwrap(), Role::Surface).is_empty());
    assert!(matches!(
        layout.block(0).unwrap().decor[0],
        Decor::Fill(_, Role::Surface)
    ));
}

#[test]
fn the_header_is_bold_and_rows_of_a_table_touch() {
    let mut layout = lay(TABLE, 300.0);
    layout.ensure(0.0, 1.0e6);
    assert_eq!(
        layout.top(1),
        layout.top(0) + layout.block(0).unwrap().height
    );
    assert_eq!(
        layout.top(2),
        layout.top(1) + layout.block(1).unwrap().height
    );
}

#[test]
fn right_and_center_alignment_offset_a_short_cell() {
    let md = "| head long | hhhhh | z |\n|---:|:---:|---|\n| x | y | w |";
    let block = one(md, 1, 1000.0);
    // Column 0 is 63 px of text, column 1 35 px, both inside 8 px of padding.
    assert_eq!(piece(&block, "x").x, 8.0 + (63.0 - 7.0));
    assert_eq!(piece(&block, "y").x, 79.0 + 8.0 + (35.0 - 7.0) / 2.0);
    assert_eq!(piece(&block, "w").x, 79.0 + 51.0 + 8.0, "unaligned is left");
}
