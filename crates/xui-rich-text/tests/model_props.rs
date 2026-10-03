//! Property tests: random edit sequences keep the invariants, every op's
//! inverse restores the document exactly, and history and fragments round-trip.

#[path = "model_common.rs"]
mod common;

use std::time::Duration;

use common::object;
use proptest::prelude::*;
use xui_rich_text::model::{
    Align, BlockKind, CharStylePatch, DocPos, DocRange, Document, EditContext, EditOp, History,
    ParaStylePatch, Selection,
};

const ALPHABET: [char; 9] = ['a', 'b', ' ', 'é', '\n', '😀', '\u{301}', '\u{200D}', 'z'];

fn text() -> impl Strategy<Value = String> {
    prop::collection::vec(prop::sample::select(ALPHABET.to_vec()), 0..6)
        .prop_map(|chars| chars.into_iter().collect())
}

#[derive(Clone, Debug)]
enum Spec {
    Insert(usize, usize, String),
    Delete(usize, usize, usize, usize),
    Split(usize, usize),
    Merge(usize),
    Style(usize, usize, usize, usize, u8),
    Para(usize, usize, u8),
    Object(usize, usize, u8),
    SetObject(usize, u8),
}

fn spec() -> impl Strategy<Value = Spec> {
    let n = 0..64usize;
    prop_oneof![
        3 => (n.clone(), n.clone(), text()).prop_map(|(p, c, t)| Spec::Insert(p, c, t)),
        3 => (n.clone(), n.clone(), n.clone(), n.clone()).prop_map(|(a, b, c, d)| Spec::Delete(a, b, c, d)),
        1 => (n.clone(), n.clone()).prop_map(|(p, c)| Spec::Split(p, c)),
        1 => n.clone().prop_map(Spec::Merge),
        2 => (n.clone(), n.clone(), n.clone(), n.clone(), 0..6u8).prop_map(|(a, b, c, d, k)| Spec::Style(a, b, c, d, k)),
        1 => (n.clone(), n.clone(), 0..4u8).prop_map(|(a, b, k)| Spec::Para(a, b, k)),
        1 => (n.clone(), n.clone(), 0..8u8).prop_map(|(p, c, s)| Spec::Object(p, c, s)),
        1 => (n, 0..8u8).prop_map(|(i, s)| Spec::SetObject(i, s)),
    ]
}

fn pos(doc: &Document, p: usize, c: usize) -> DocPos {
    let para = p % doc.paragraph_count();
    let text = doc.paragraphs()[para].text();
    let mut stops: Vec<usize> = text.char_indices().map(|(i, _)| i).collect();
    stops.push(text.len());
    DocPos::new(para, stops[c % stops.len()])
}

fn resolve(doc: &Document, spec: &Spec) -> Option<EditOp> {
    Some(match spec {
        Spec::Insert(p, c, t) => EditOp::InsertText {
            at: pos(doc, *p, *c),
            text: t.clone(),
            style: None,
        },
        Spec::Delete(a, b, c, d) => EditOp::Delete {
            range: DocRange::new(pos(doc, *a, *b), pos(doc, *c, *d)),
        },
        Spec::Split(p, c) => EditOp::SplitParagraph {
            at: pos(doc, *p, *c),
        },
        Spec::Merge(p) => EditOp::MergeParagraph {
            para: *p % doc.paragraph_count().checked_sub(1).filter(|&n| n > 0)?,
        },
        Spec::Style(a, b, c, d, kind) => EditOp::SetCharStyle {
            range: DocRange::new(pos(doc, *a, *b), pos(doc, *c, *d)),
            patch: match kind {
                0 => CharStylePatch::bold(true),
                1 => CharStylePatch::bold(false),
                2 => CharStylePatch::italic(true),
                3 => CharStylePatch::underline(true),
                4 => CharStylePatch::strike(true),
                _ => CharStylePatch::italic(false),
            },
        },
        Spec::Para(a, b, kind) => {
            let n = doc.paragraph_count();
            let (a, b) = (a % n, b % n);
            EditOp::SetParaStyle {
                paras: a.min(b)..a.max(b) + 1,
                patch: match kind {
                    0 => ParaStylePatch::align(Align::Center),
                    1 => ParaStylePatch::kind(BlockKind::Heading(2)),
                    2 => ParaStylePatch::align(Align::Left),
                    _ => ParaStylePatch::kind(BlockKind::Body),
                },
            }
        }
        Spec::Object(p, c, s) => EditOp::InsertObject {
            at: pos(doc, *p, *c),
            object: object(*s),
        },
        Spec::SetObject(i, s) => {
            let ids: Vec<_> = doc
                .paragraphs()
                .iter()
                .flat_map(|p| p.anchors().iter().copied())
                .collect();
            if ids.is_empty() {
                return None;
            }
            EditOp::SetObject {
                id: ids[i % ids.len()],
                object: object(*s),
            }
        }
    })
}

fn start_doc(text: &str) -> Document {
    Document::from_plain_text(text)
}

proptest! {
    #[test]
    fn ops_keep_invariants_and_invert_exactly(
        initial in text(),
        specs in prop::collection::vec(spec(), 1..24),
    ) {
        let mut doc = start_doc(&initial);
        let first = doc.clone();
        let mut inverses = Vec::new();
        for spec in &specs {
            let Some(op) = resolve(&doc, spec) else { continue };
            let before = doc.clone();
            let inverse = doc.apply(op).expect("a resolved op applies");
            prop_assert_eq!(doc.check(), Ok(()));
            let mut undone = doc.clone();
            undone.apply(inverse.clone()).expect("the inverse applies");
            prop_assert_eq!(undone.check(), Ok(()));
            prop_assert_eq!(undone.paragraphs(), before.paragraphs());
            prop_assert!(undone.content_eq(&before));
            inverses.push(inverse);
        }
        for inverse in inverses.into_iter().rev() {
            doc.apply(inverse).expect("the inverse applies");
        }
        prop_assert_eq!(doc.paragraphs(), first.paragraphs());
    }

    #[test]
    fn history_undoes_and_redoes_everything(
        initial in text(),
        specs in prop::collection::vec(spec(), 1..24),
        gaps in prop::collection::vec(0..3000u64, 24),
    ) {
        let mut doc = start_doc(&initial);
        let first = doc.clone();
        let mut history = History::new();
        let mut now = Duration::ZERO;
        let mut caret = Selection::caret(DocPos::default());
        for (spec, gap) in specs.iter().zip(&gaps) {
            let Some(op) = resolve(&doc, spec) else { continue };
            now += Duration::from_millis(*gap);
            let after = Selection::caret(match &op {
                EditOp::InsertText { at, text, .. } => {
                    DocPos::new(at.para, at.byte + text.len())
                }
                EditOp::Delete { range } => range.start,
                _ => DocPos::default(),
            });
            let ctx = EditContext::new(caret, after).at(now);
            history.edit(&mut doc, op, ctx).expect("a resolved op applies");
            caret = after;
        }
        let last = doc.clone();
        while history.undo(&mut doc).is_some() {
            prop_assert_eq!(doc.check(), Ok(()));
        }
        prop_assert_eq!(doc.paragraphs(), first.paragraphs());
        while history.redo(&mut doc).is_some() {
            prop_assert_eq!(doc.check(), Ok(()));
        }
        prop_assert_eq!(doc.paragraphs(), last.paragraphs());
        prop_assert!(!history.can_redo());
    }

    #[test]
    fn fragments_round_trip(
        initial in text(),
        specs in prop::collection::vec(spec(), 0..16),
        (a, b, c, d, e, f) in (0..64usize, 0..64usize, 0..64usize, 0..64usize, 0..64usize, 0..64usize),
    ) {
        let mut src = start_doc(&initial);
        for spec in &specs {
            if let Some(op) = resolve(&src, spec) {
                src.apply(op).unwrap();
            }
        }
        let range = DocRange::new(pos(&src, a, b), pos(&src, c, d));
        let fragment = src.extract_fragment(range).unwrap();
        prop_assert_eq!(fragment.to_plain_text(), src.text_in(range).unwrap());

        let mut dst = src.clone();
        let before = dst.clone();
        let at = pos(&dst, e, f);
        let (inverse, end) = dst.insert_fragment(at, &fragment).unwrap();
        prop_assert_eq!(dst.check(), Ok(()));
        let inserted = dst.text_in(DocRange::new(at, end)).unwrap();
        prop_assert_eq!(inserted, fragment.to_plain_text());
        dst.apply(inverse).unwrap();
        prop_assert!(dst.content_eq(&before));

        let mut copy = src.clone();
        let at = pos(&copy, e, f);
        let (_, end) = copy.insert_fragment(at, &fragment).unwrap();
        let again = copy.extract_fragment(DocRange::new(at, end)).unwrap();
        prop_assert_eq!(again.to_plain_text(), fragment.to_plain_text());
    }
}
