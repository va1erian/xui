#![forbid(unsafe_code)]

//! Markdown source to a [`Document`], through pulldown-cmark's events.

mod builder;
mod events;
mod slug;

use pulldown_cmark::{Options, Parser};

use crate::model::Document;
use builder::Builder;

/// Text offsets are `u32`, so a document is cut at this many source bytes.
const MAX_SOURCE: usize = u32::MAX as usize;

impl Document {
    /// Parses `markdown` (CommonMark with GFM tables, strike-through and task
    /// lists). Raw HTML is kept as literal text; an image is its alt text, its
    /// source listed in [`Document::images`].
    pub fn parse(markdown: &str) -> Document {
        Document::parse_limited(markdown, MAX_SOURCE).0
    }

    /// Parses at most `max_bytes` of `markdown`, cut at a block boundary: the
    /// last blank line, else the last line end, else the last character
    /// boundary within the limit. Returns whether anything was left out, so
    /// the app can say so.
    pub fn parse_limited(markdown: &str, max_bytes: usize) -> (Document, bool) {
        let max_bytes = max_bytes.min(MAX_SOURCE);
        if markdown.len() <= max_bytes {
            return (build(markdown), false);
        }
        let mut end = max_bytes;
        while !markdown.is_char_boundary(end) {
            end -= 1;
        }
        let head = &markdown[..end];
        let cut = head
            .rfind("\n\n")
            .filter(|&i| i > 0)
            .or_else(|| head.rfind('\n').filter(|&i| i > 0))
            .unwrap_or(end);
        (build(&markdown[..cut]), true)
    }
}

fn build(markdown: &str) -> Document {
    let options =
        Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS;
    let mut builder = Builder::default();
    for event in Parser::new_ext(markdown, options) {
        builder.event(event);
    }
    builder.finish()
}
