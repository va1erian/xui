//! Tables in the JSON save format and the Markdown export.
#![cfg(feature = "serde")]

#[path = "edit_common.rs"]
mod harness;

use harness::Ed;
use serde_json::{Value, json};
use xui_rich_text::edit::Command;
use xui_rich_text::format::{FormatError, ImageExport, from_json, to_json, to_markdown};
use xui_rich_text::model::Document;

/// "intro", a 2 x 2 table of a | b / c | d with a second paragraph in b, and
/// "outro".
fn table_doc() -> Document {
    let mut ed = Ed::new("introouto");
    ed.caret_at(0, 5);
    ed.run(Command::InsertTable {
        rows: 2,
        columns: 2,
    });
    ed.type_str("a|1");
    ed.run(Command::NextCell);
    ed.type_str("b");
    ed.run(Command::InsertParagraph);
    ed.type_str("b2");
    for text in ["c", "d"] {
        ed.run(Command::NextCell);
        ed.type_str(text);
    }
    let cursor = ed.state.table_cursor().unwrap();
    let mut table = cursor.table;
    table.header = true;
    ed.run(Command::SetTable {
        id: cursor.id,
        table: table.with_widths(&[3.0, 1.0]),
    });
    ed.state.doc
}

#[test]
fn tables_round_trip() {
    let doc = table_doc();
    let back = from_json(&to_json(&doc)).unwrap();
    assert!(back.content_eq(&doc));
    let span = &back.table_spans()[0];
    let table = back.tables().get(span.id).unwrap();
    assert!(table.header && table.border);
    assert!((table.columns[0] - 0.75).abs() < 1e-6);
}

#[test]
fn a_document_without_tables_writes_no_table_fields() {
    let json: Value = serde_json::from_str(&to_json(&Document::from_plain_text("x"))).unwrap();
    assert!(json.get("tables").is_none());
    assert!(json["paragraphs"][0].get("cell").is_none());
    assert_eq!(json["version"], 1);
}

#[test]
fn broken_tables_are_refused() {
    let base: Value = serde_json::from_str(&to_json(&table_doc())).unwrap();
    let broken = |edit: &dyn Fn(&mut Value)| {
        let mut file = base.clone();
        edit(&mut file);
        from_json(&file.to_string())
    };
    // A cell of a table that is not there.
    let unknown = broken(&|f| f["tables"] = json!([]));
    assert!(matches!(unknown, Err(FormatError::Invalid(_))));
    // A row with one cell too few.
    let short = broken(&|f| f["paragraphs"][2]["cell"]["start"] = json!("continue"));
    assert!(matches!(short, Err(FormatError::Invalid(_))));
    // No paragraph after the table.
    let last = broken(&|f| {
        let paras = f["paragraphs"].as_array_mut().unwrap();
        paras.pop();
    });
    assert!(matches!(last, Err(FormatError::Invalid(_))));
    // Widths that do not add up.
    let widths = broken(&|f| f["tables"][0]["columns"] = json!([0.5, 0.9]));
    assert!(matches!(widths, Err(FormatError::Invalid(_))));
    let duplicate = broken(&|f| {
        let table = f["tables"][0].clone();
        f["tables"].as_array_mut().unwrap().push(table);
    });
    assert!(matches!(duplicate, Err(FormatError::Invalid(_))));
}

#[test]
fn markdown_writes_a_pipe_table() {
    let md = to_markdown(&table_doc(), &ImageExport::DataUri);
    assert_eq!(
        md,
        "intro\n\n| a\\|1 | b<br>b2 |\n| --- | --- |\n| c | d |\n\nouto\n"
    );
}
