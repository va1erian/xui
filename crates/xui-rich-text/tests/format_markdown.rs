//! Markdown export: golden strings per mapping row.

#[path = "model_common.rs"]
mod common;

use std::cell::RefCell;
use std::rc::Rc;

use common::{apply, object, range};
use xui_core::Color;
use xui_rich_text::format::{ImageExport, to_markdown};
use xui_rich_text::model::{
    BlockKind, CharStylePatch, DocPos, Document, EditOp, ListItem, ListKind, ParaStylePatch,
    TextColor,
};

fn md(doc: &Document) -> String {
    to_markdown(doc, &ImageExport::DataUri)
}

fn doc(text: &str) -> Document {
    Document::from_plain_text(text)
}

fn style(doc: &mut Document, r: (usize, usize, usize, usize), patch: CharStylePatch) {
    apply(
        doc,
        EditOp::SetCharStyle {
            range: range(r.0, r.1, r.2, r.3),
            patch,
        },
    );
}

fn para(doc: &mut Document, index: usize, patch: ParaStylePatch) {
    apply(
        doc,
        EditOp::SetParaStyle {
            paras: index..index + 1,
            patch,
        },
    );
}

fn item(doc: &mut Document, index: usize, kind: ListKind, level: u8) {
    para(
        doc,
        index,
        ParaStylePatch::list(Some(ListItem { kind, level })),
    );
}

#[test]
fn paragraphs_are_separated_by_blank_lines() {
    assert_eq!(md(&doc("one\ntwo")), "one\n\ntwo\n");
    assert_eq!(md(&doc("one\n\n\ntwo")), "one\n\ntwo\n");
    assert_eq!(md(&doc("")), "");
}

#[test]
fn headings_and_quotes() {
    let mut d = doc("Big\nSmall\nbody\nq1\nq2\nafter");
    para(&mut d, 0, ParaStylePatch::kind(BlockKind::Heading(1)));
    para(&mut d, 1, ParaStylePatch::kind(BlockKind::Heading(3)));
    para(&mut d, 3, ParaStylePatch::kind(BlockKind::Quote));
    para(&mut d, 4, ParaStylePatch::kind(BlockKind::Quote));
    assert_eq!(
        md(&d),
        "# Big\n\n### Small\n\nbody\n\n> q1\n>\n> q2\n\nafter\n"
    );
}

#[test]
fn emphasis_kinds() {
    let mut d = doc("a bold b italic c strike d");
    style(&mut d, (0, 2, 0, 6), CharStylePatch::bold(true));
    style(&mut d, (0, 9, 0, 15), CharStylePatch::italic(true));
    style(&mut d, (0, 18, 0, 24), CharStylePatch::strike(true));
    assert_eq!(md(&d), "a **bold** b *italic* c ~~strike~~ d\n");
}

#[test]
fn dropped_attributes_leave_plain_text() {
    let mut d = doc("plain");
    style(&mut d, (0, 0, 0, 5), CharStylePatch::underline(true));
    style(
        &mut d,
        (0, 0, 0, 5),
        CharStylePatch {
            color: Some(TextColor::Fixed(Color::rgb(9, 9, 9))),
            highlight: Some(Some(Color::rgb(1, 1, 1))),
            ..CharStylePatch::default()
        },
    );
    assert_eq!(md(&d), "plain\n");
}

#[test]
fn overlapping_emphasis_nests_instead_of_crossing() {
    let mut d = doc("abcd");
    style(&mut d, (0, 0, 0, 3), CharStylePatch::bold(true));
    style(&mut d, (0, 1, 0, 4), CharStylePatch::italic(true));
    assert_eq!(md(&d), "**a*bc***_d_\n");

    let mut d = doc("abcd");
    style(&mut d, (0, 0, 0, 4), CharStylePatch::bold(true));
    style(&mut d, (0, 1, 0, 3), CharStylePatch::italic(true));
    assert_eq!(md(&d), "**a*bc*d**\n");

    let mut d = doc("ab");
    style(&mut d, (0, 0, 0, 2), CharStylePatch::bold(true));
    style(&mut d, (0, 0, 0, 2), CharStylePatch::italic(true));
    assert_eq!(md(&d), "***ab***\n");
}

#[test]
fn markers_stay_off_whitespace() {
    let mut d = doc("a  bold  b");
    style(&mut d, (0, 1, 0, 9), CharStylePatch::bold(true));
    assert_eq!(md(&d), "a  **bold**  b\n");
    let mut d = doc("x   y");
    style(&mut d, (0, 1, 0, 4), CharStylePatch::bold(true));
    assert_eq!(md(&d), "x   y\n");
}

#[test]
fn links() {
    let mut d = doc("see here now");
    let link = |url: &str| CharStylePatch {
        link: Some(Some(url.into())),
        ..CharStylePatch::default()
    };
    style(&mut d, (0, 4, 0, 8), link("https://x.org/a"));
    assert_eq!(md(&d), "see [here](https://x.org/a) now\n");
    style(&mut d, (0, 4, 0, 6), CharStylePatch::bold(true));
    assert_eq!(md(&d), "see [**he**re](https://x.org/a) now\n");
    style(&mut d, (0, 4, 0, 8), link("a b(c)"));
    assert_eq!(md(&d), "see [**he**re](<a b(c)>) now\n");
}

#[test]
fn escaping() {
    assert_eq!(
        md(&doc("1*2 _x_ [a](b) `c` <d> a|b ~~ \\ &")),
        "1\\*2 \\_x\\_ \\[a\\](b) \\`c\\` \\<d\\> a\\|b \\~\\~ \\\\ \\&\n"
    );
    assert_eq!(md(&doc("# not a heading")), "\\# not a heading\n");
    assert_eq!(md(&doc("- not a list")), "\\- not a list\n");
    assert_eq!(md(&doc("1. not a list")), "1\\. not a list\n");
    assert_eq!(md(&doc("12) not")), "12\\) not\n");
    assert_eq!(md(&doc("a - b 1. c")), "a - b 1. c\n");
}

#[test]
fn line_breaks_become_backslash_newlines() {
    let mut d = doc("");
    apply(
        &mut d,
        EditOp::InsertText {
            at: DocPos::new(0, 0),
            text: "one\u{2028}two\u{2028}- three".into(),
            style: None,
        },
    );
    assert_eq!(md(&d), "one\\\ntwo\\\n\\- three\n");
    para(&mut d, 0, ParaStylePatch::kind(BlockKind::Heading(2)));
    assert_eq!(md(&d), "## one two - three\n");
    para(&mut d, 0, ParaStylePatch::kind(BlockKind::Body));
    item(&mut d, 0, ListKind::Bullet, 0);
    assert_eq!(md(&d), "- one\\\n  two\\\n  \\- three\n");
}

#[test]
fn lists_nest_by_level() {
    let mut d = doc("a\nb\nc\nd\ne\nf\ng\nplain");
    item(&mut d, 0, ListKind::Bullet, 0);
    item(&mut d, 1, ListKind::Bullet, 1);
    item(&mut d, 2, ListKind::Numbered, 2);
    item(&mut d, 3, ListKind::Numbered, 2);
    item(&mut d, 4, ListKind::Bullet, 0);
    item(&mut d, 5, ListKind::Numbered, 0);
    item(&mut d, 6, ListKind::Numbered, 0);
    assert_eq!(
        md(&d),
        "- a\n  - b\n    1. c\n    2. d\n- e\n1. f\n2. g\n\nplain\n"
    );
}

#[test]
fn empty_items_stay_and_empty_paragraphs_vanish() {
    let mut d = doc("a\n\nb");
    item(&mut d, 0, ListKind::Bullet, 0);
    item(&mut d, 1, ListKind::Bullet, 0);
    item(&mut d, 2, ListKind::Bullet, 0);
    assert_eq!(md(&d), "- a\n-\n- b\n");
}

#[test]
fn images_as_data_uris_and_callbacks() {
    let mut d = doc("ab");
    apply(
        &mut d,
        EditOp::InsertObject {
            at: DocPos::new(0, 1),
            object: object(2),
        },
    );
    let uri = md(&d);
    assert!(
        uri.starts_with("a![image 2](data:image/png;base64,iVBOR"),
        "{uri}"
    );
    assert!(uri.ends_with(")b\n"), "{uri}");

    let seen = Rc::new(RefCell::new(Vec::new()));
    let log = Rc::clone(&seen);
    let export = ImageExport::Callback(Box::new(move |image, alt| {
        log.borrow_mut().push((image.size(), alt.to_owned()));
        format!("doc images/{}.png", log.borrow().len())
    }));
    assert_eq!(
        to_markdown(&d, &export),
        "a![image 2](<doc images/1.png>)b\n"
    );
    assert_eq!(*seen.borrow(), vec![((3, 2), "image 2".to_owned())]);

    let plain = ImageExport::Callback(Box::new(|_, _| "pics/1.png".into()));
    style_alt(&mut d);
    assert_eq!(to_markdown(&d, &plain), "a![a\\]b](pics/1.png)b\n");
}

fn style_alt(d: &mut Document) {
    let id = d.paragraphs()[0].anchors()[0];
    let mut image = object(2);
    image.alt = "a]b".into();
    apply(d, EditOp::SetObject { id, object: image });
}

proptest::proptest! {
    #[test]
    fn export_of_random_documents_is_well_formed(
        initial in common::text(),
        specs in proptest::collection::vec(common::spec(), 0..24),
    ) {
        let mut d = common::start_doc(&initial);
        for spec in &specs {
            if let Some(op) = common::resolve(&d, spec) {
                d.apply(op).unwrap();
            }
        }
        let out = md(&d);
        proptest::prop_assert!(out.is_empty() || out.ends_with('\n'));
        proptest::prop_assert!(!out.contains("\n\n\n"));
    }
}
