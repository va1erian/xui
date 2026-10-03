#![forbid(unsafe_code)]

//! Caret movement over text: grapheme and word boundaries within a paragraph,
//! and document-level steps that cross paragraph boundaries.
//!
//! All offsets are byte offsets on `char` boundaries (grapheme boundaries for
//! what these functions return).

use std::ops::Range;

use unicode_segmentation::UnicodeSegmentation;

use super::{DocPos, Document};

/// The grapheme boundary after `byte`, or the text length at the end.
pub fn next_grapheme(text: &str, byte: usize) -> usize {
    text.grapheme_indices(true)
        .map(|(start, g)| start + g.len())
        .find(|&end| end > byte)
        .unwrap_or(text.len())
}

/// The grapheme boundary before `byte`, or 0 at the start.
pub fn prev_grapheme(text: &str, byte: usize) -> usize {
    text.grapheme_indices(true)
        .map(|(start, _)| start)
        .take_while(|&start| start < byte)
        .last()
        .unwrap_or(0)
}

fn is_blank(segment: &str) -> bool {
    segment.chars().all(char::is_whitespace)
}

/// The start of the next word after `byte` (Ctrl+Right), or the text length.
pub fn next_word(text: &str, byte: usize) -> usize {
    text.split_word_bound_indices()
        .find(|&(start, segment)| start > byte && !is_blank(segment))
        .map_or(text.len(), |(start, _)| start)
}

/// The start of the word before `byte` (Ctrl+Left), or 0.
pub fn prev_word(text: &str, byte: usize) -> usize {
    text.split_word_bound_indices()
        .rfind(|&(start, segment)| start < byte && !is_blank(segment))
        .map_or(0, |(start, _)| start)
}

/// The word (or run of blanks, or single symbol) containing `byte`, for
/// double-click selection. At the end of the text it is the last one.
pub fn word_range_at(text: &str, byte: usize) -> Range<usize> {
    let mut last = 0..0;
    for (start, segment) in text.split_word_bound_indices() {
        last = start..start + segment.len();
        if byte < last.end {
            return last;
        }
    }
    last
}

/// The position one grapheme after `pos`, crossing into the next paragraph at
/// a paragraph end; `None` at the end of the document.
pub fn next_pos(doc: &Document, pos: DocPos) -> Option<DocPos> {
    let text = doc.paragraphs().get(pos.para)?.text();
    if pos.byte < text.len() {
        Some(DocPos::new(pos.para, next_grapheme(text, pos.byte)))
    } else if pos.para + 1 < doc.paragraphs().len() {
        Some(DocPos::new(pos.para + 1, 0))
    } else {
        None
    }
}

/// The position one grapheme before `pos`, crossing into the previous
/// paragraph at a paragraph start; `None` at the start of the document.
pub fn prev_pos(doc: &Document, pos: DocPos) -> Option<DocPos> {
    let text = doc.paragraphs().get(pos.para)?.text();
    if pos.byte > 0 {
        Some(DocPos::new(pos.para, prev_grapheme(text, pos.byte)))
    } else if pos.para > 0 {
        let prev = doc.paragraphs()[pos.para - 1].text();
        Some(DocPos::new(pos.para - 1, prev.len()))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn graphemes_keep_clusters_whole() {
        let text = "a\u{1F468}\u{200D}\u{1F469}e\u{301}b";
        let zwj_end = 1 + "\u{1F468}\u{200D}\u{1F469}".len();
        assert_eq!(next_grapheme(text, 0), 1);
        assert_eq!(next_grapheme(text, 1), zwj_end);
        let combined_end = zwj_end + "e\u{301}".len();
        assert_eq!(next_grapheme(text, zwj_end), combined_end);
        assert_eq!(prev_grapheme(text, combined_end), zwj_end);
        assert_eq!(prev_grapheme(text, zwj_end), 1);
        assert_eq!(prev_grapheme(text, 0), 0);
        assert_eq!(next_grapheme(text, text.len()), text.len());
    }

    #[test]
    fn words_skip_blanks() {
        let text = "one  two, three";
        assert_eq!(next_word(text, 0), 5);
        assert_eq!(next_word(text, 5), 8);
        assert_eq!(next_word(text, 8), 10);
        assert_eq!(next_word(text, 10), text.len());
        assert_eq!(prev_word(text, text.len()), 10);
        assert_eq!(prev_word(text, 5), 0);
        assert_eq!(prev_word(text, 0), 0);
        assert_eq!(word_range_at(text, 6), 5..8);
        assert_eq!(word_range_at(text, 3), 3..5);
        assert_eq!(word_range_at(text, text.len()), 10..15);
    }

    #[test]
    fn positions_cross_paragraphs() {
        let doc = Document::from_plain_text("ab\n\ncd");
        assert_eq!(next_pos(&doc, DocPos::new(0, 2)), Some(DocPos::new(1, 0)));
        assert_eq!(next_pos(&doc, DocPos::new(1, 0)), Some(DocPos::new(2, 0)));
        assert_eq!(next_pos(&doc, DocPos::new(2, 2)), None);
        assert_eq!(prev_pos(&doc, DocPos::new(2, 0)), Some(DocPos::new(1, 0)));
        assert_eq!(prev_pos(&doc, DocPos::new(1, 0)), Some(DocPos::new(0, 2)));
        assert_eq!(prev_pos(&doc, DocPos::new(0, 0)), None);
    }
}
