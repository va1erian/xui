//! Page breaks and the page setup: commands, undo and the save format.

#[path = "edit_common.rs"]
mod harness;

use harness::{Ed, undo};
use serde_json::Value;
use xui_core::Dip;
use xui_rich_text::Document;
use xui_rich_text::edit::Command;
use xui_rich_text::format::{FormatError, from_json, to_json};
use xui_rich_text::model::{EditError, EditOp, PageSetup};

fn breaks(ed: &Ed) -> Vec<bool> {
    let doc = &ed.state.doc;
    doc.paragraphs()
        .iter()
        .map(|p| doc.styles().para(p.style()).page_break_before)
        .collect()
}

#[test]
fn ctrl_enter_starts_the_second_half_on_a_new_page() {
    let mut ed = Ed::new("onetwo");
    ed.caret_at(0, 3);
    ed.run(Command::InsertPageBreak);
    assert_eq!(ed.text(), "one\ntwo");
    assert_eq!(breaks(&ed), [false, true]);
    assert_eq!(ed.caret(), xui_rich_text::DocPos::new(1, 0));
    undo(&mut ed);
    assert_eq!(ed.text(), "onetwo");
    assert_eq!(breaks(&ed), [false]);
}

#[test]
fn enter_and_pasted_lines_do_not_carry_the_break_on() {
    let mut ed = Ed::new("one");
    ed.caret_at(0, 3);
    ed.run(Command::InsertPageBreak);
    ed.type_str("two");
    ed.run(Command::InsertParagraph);
    ed.type_str("three");
    assert_eq!(breaks(&ed), [false, true, false]);
    // Enter at the start of the break paragraph keeps the break on the
    // (now empty) paragraph that starts the page.
    ed.caret_at(1, 0);
    ed.run(Command::InsertParagraph);
    assert_eq!(breaks(&ed), [false, true, false, false]);
    ed.caret_at(1, 0);
    ed.run(Command::InsertText("a\nb".into()));
    assert_eq!(breaks(&ed), [false, true, false, false, false]);
}

#[test]
fn backspace_at_a_page_start_removes_the_break_first() {
    let mut ed = Ed::new("one");
    ed.caret_at(0, 3);
    ed.run(Command::InsertPageBreak);
    ed.type_str("two");
    ed.caret_at(1, 0);
    ed.run(Command::Backspace);
    assert_eq!(ed.text(), "one\ntwo", "the text stays");
    assert_eq!(breaks(&ed), [false, false]);
    ed.run(Command::Backspace);
    assert_eq!(ed.text(), "onetwo", "then the paragraphs merge");
}

#[test]
fn the_page_setup_is_one_undoable_step() {
    let mut ed = Ed::new("text");
    assert_eq!(*ed.state.doc.page(), PageSetup::a4());
    let letter = PageSetup::letter().oriented(true);
    let effect = ed.run(Command::SetPageSetup(letter));
    assert!(effect.doc_changed());
    assert_eq!(*ed.state.doc.page(), letter);
    assert!(
        !ed.run(Command::SetPageSetup(letter)).doc_changed(),
        "no change, no step"
    );
    undo(&mut ed);
    assert_eq!(*ed.state.doc.page(), PageSetup::a4());
}

#[test]
fn a_page_with_no_room_for_text_is_refused() {
    let mut doc = Document::new();
    let bad = PageSetup::a4().with_margins(Dip(500.0));
    assert_eq!(
        doc.apply(EditOp::SetPage(bad)).unwrap_err(),
        EditError::BadPage
    );
    assert_eq!(*doc.page(), PageSetup::a4());
}

#[test]
fn page_setup_and_breaks_round_trip_through_json() {
    let mut ed = Ed::new("onetwo");
    ed.caret_at(0, 3);
    ed.run(Command::InsertPageBreak);
    ed.run(Command::SetPageSetup(PageSetup::letter()));
    let json = to_json(&ed.state.doc);
    let loaded = from_json(&json).unwrap();
    assert_eq!(*loaded.page(), PageSetup::letter());
    let style = loaded.styles().para(loaded.paragraphs()[1].style());
    assert!(style.page_break_before);
    assert_eq!(to_json(&loaded), json);
}

#[test]
fn files_from_before_page_view_load_on_a4() {
    let mut value: Value =
        serde_json::from_str(&to_json(&Document::from_plain_text("a\nb"))).unwrap();
    value.as_object_mut().unwrap().remove("page");
    let doc = from_json(&value.to_string()).unwrap();
    assert_eq!(*doc.page(), PageSetup::a4());
    // And a paragraph style without the flag has no break.
    assert!(!doc.styles().paras()[0].page_break_before);
    assert!(
        !value.to_string().contains("page_break_before"),
        "written only when set"
    );
}

#[test]
fn a_bad_page_in_a_file_is_an_error() {
    let mut value: Value = serde_json::from_str(&to_json(&Document::new())).unwrap();
    value["page"]["margins"] = serde_json::json!([500, 0, 500, 0]);
    assert!(matches!(
        from_json(&value.to_string()),
        Err(FormatError::Invalid(_))
    ));
}
