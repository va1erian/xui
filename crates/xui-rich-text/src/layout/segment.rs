#![forbid(unsafe_code)]

//! Segmentation: a paragraph's text cut into pieces at style boundaries,
//! word/space transitions and the UAX #14 line-break opportunities.

use std::ops::Range;

use unicode_linebreak::{BreakOpportunity, linebreaks};

use crate::model::{CharStyleId, OBJECT_CHAR, ObjectId, Paragraph};

/// Whether a line may or must break after a piece.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Brk {
    /// No break opportunity: the piece sticks to the next one.
    None,
    /// A line may break here.
    Allowed,
    /// A line must break here.
    Mandatory,
}

/// What a piece is made of.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SegKind {
    /// Non-space text.
    Word,
    /// Spaces and tabs.
    Space,
    /// An anchored image.
    Object(ObjectId),
    /// A forced line break.
    Break,
}

/// A piece of uniform style between two cuts.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Segment {
    pub range: Range<usize>,
    pub kind: SegKind,
    pub style: CharStyleId,
    pub brk: Brk,
}

/// The kind of a character, or `None` for an object anchor.
fn kind_of(c: char) -> Option<SegKind> {
    match c {
        '\n' | '\u{2028}' | '\u{2029}' => Some(SegKind::Break),
        ' ' | '\t' => Some(SegKind::Space),
        OBJECT_CHAR => None,
        _ => Some(SegKind::Word),
    }
}

/// Cuts `para` into pieces. The pieces tile the text; an empty paragraph has
/// none.
pub(crate) fn segment(para: &Paragraph) -> Vec<Segment> {
    let text = para.text();
    let opportunities: Vec<(usize, BreakOpportunity)> = linebreaks(text)
        .filter(|&(at, _)| at < text.len())
        .collect();
    let opportunity_at = |at: usize| {
        opportunities
            .binary_search_by_key(&at, |&(pos, _)| pos)
            .ok()
            .map(|i| opportunities[i].1)
    };
    let mut anchors = para.anchors().iter();
    let mut out: Vec<Segment> = Vec::new();
    for (range, style) in para.runs() {
        let mut open: Option<Segment> = None;
        for (offset, c) in text[range.clone()].char_indices() {
            let at = range.start + offset;
            let kind = kind_of(c).unwrap_or_else(|| {
                SegKind::Object(*anchors.next().expect("every object char has an anchor"))
            });
            let extends = open.as_ref().is_some_and(|s| {
                s.kind == kind
                    && matches!(kind, SegKind::Word | SegKind::Space)
                    && opportunity_at(at).is_none()
            });
            if extends {
                if let Some(s) = open.as_mut() {
                    s.range.end = at + c.len_utf8();
                }
                continue;
            }
            out.extend(open.take());
            open = Some(Segment {
                range: at..at + c.len_utf8(),
                kind,
                style,
                brk: Brk::None,
            });
        }
        out.extend(open.take());
    }
    for seg in &mut out {
        seg.brk = match (opportunity_at(seg.range.end), seg.kind) {
            (_, SegKind::Break) | (Some(BreakOpportunity::Mandatory), _) => Brk::Mandatory,
            (Some(BreakOpportunity::Allowed), _) => Brk::Allowed,
            (None, _) => Brk::None,
        };
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ParaStyleId;

    fn pieces(text: &str) -> Vec<(&str, Brk)> {
        let para = Paragraph::new(text, ParaStyleId::DEFAULT, CharStyleId::DEFAULT);
        segment(&para)
            .into_iter()
            .map(|s| (&text[s.range], s.brk))
            .collect()
    }

    #[test]
    fn words_and_spaces_break_after_the_space() {
        assert_eq!(
            pieces("ab  cd"),
            [("ab", Brk::None), ("  ", Brk::Allowed), ("cd", Brk::None)]
        );
    }

    #[test]
    fn a_hyphen_allows_a_break() {
        assert_eq!(pieces("a-b"), [("a-", Brk::Allowed), ("b", Brk::None)]);
    }

    #[test]
    fn a_line_separator_is_a_mandatory_break() {
        let got = pieces("a\u{2028}b");
        assert_eq!(got[1], ("\u{2028}", Brk::Mandatory));
    }

    #[test]
    fn an_empty_paragraph_has_no_pieces() {
        assert!(pieces("").is_empty());
    }
}
