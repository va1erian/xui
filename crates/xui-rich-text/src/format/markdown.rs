#![forbid(unsafe_code)]

//! Lossy GitHub Flavored Markdown export, for sharing basic documents.
//!
//! | Model | Markdown |
//! |---|---|
//! | `Heading(n)`, `Quote` | `#`×n, `> ` |
//! | bold / italic / strike | `**`, `*`, `~~` |
//! | link | `[text](url)` |
//! | bullet / numbered list | `- ` / `1. `, indented by level |
//! | line break (U+2028) | backslash, newline |
//! | image | `![alt](target)` |
//!
//! Everything else (underline, colour, highlight, size, font, alignment,
//! indents, spacing, wrap) is dropped. The crate does no file I/O: where an
//! image goes is the caller's [`ImageExport`] choice.

mod inline;

use xui_core::Image;

use crate::model::{BlockKind, Document, ListKind};

/// Chooses the target of an exported image from the image and its alt text.
pub type ImageTarget = dyn Fn(&Image, &str) -> String;

/// Where exported images point.
pub enum ImageExport {
    /// Embed each image as a `data:image/png;base64,...` URI, so the file is
    /// self-contained.
    DataUri,
    /// Ask the caller for the target given the image and its alt text; the app
    /// can write `doc_images/1.png` beside the `.md` and return that path.
    Callback(Box<ImageTarget>),
}

/// One open list level while writing consecutive list items.
struct Level {
    kind: ListKind,
    count: usize,
}

impl Level {
    /// The columns the marker takes, which nested content is indented by.
    fn width(&self) -> usize {
        match self.kind {
            ListKind::Bullet => 2,
            ListKind::Numbered => self.count.to_string().len() + 2,
        }
    }
}

#[derive(PartialEq)]
enum Block {
    List,
    Quote,
    Other,
}

/// Exports `doc` as GFM Markdown, ending in a newline unless it is empty.
pub fn to_markdown(doc: &Document, images: &ImageExport) -> String {
    let mut out = String::new();
    let mut levels: Vec<Level> = Vec::new();
    let mut previous: Option<Block> = None;
    for para in doc.paragraphs() {
        let style = doc.styles().para(para.style());
        let heading = match style.kind {
            BlockKind::Heading(n) => Some(n.clamp(1, 6) as usize),
            _ => None,
        };
        let body = inline::render(inline::units(doc, para, images, heading.is_some()));
        if body.trim().is_empty() && style.list.is_none() {
            continue;
        }

        let quote = if style.kind == BlockKind::Quote {
            "> "
        } else {
            ""
        };
        let mut first = String::from(quote);
        let mut rest = String::from(quote);
        if let Some(item) = style.list {
            let level = item.level as usize;
            levels.truncate(level + 1);
            while levels.len() < level {
                levels.push(Level {
                    kind: ListKind::Bullet,
                    count: 0,
                });
            }
            match levels.get_mut(level) {
                Some(l) if l.kind == item.kind => l.count += 1,
                Some(l) => {
                    *l = Level {
                        kind: item.kind,
                        count: 1,
                    }
                }
                None => levels.push(Level {
                    kind: item.kind,
                    count: 1,
                }),
            }
            let indent: usize = levels[..level].iter().map(Level::width).sum();
            let marker = match item.kind {
                ListKind::Bullet => "- ".to_owned(),
                ListKind::Numbered => format!("{}. ", levels[level].count),
            };
            first.push_str(&" ".repeat(indent));
            first.push_str(&marker);
            rest.push_str(&" ".repeat(indent + marker.len()));
        } else {
            levels.clear();
        }
        if let Some(n) = heading {
            first.push_str(&"#".repeat(n));
            first.push(' ');
        }

        let block = match (style.list, style.kind) {
            (Some(_), _) => Block::List,
            (None, BlockKind::Quote) => Block::Quote,
            _ => Block::Other,
        };
        match (&previous, &block) {
            (None, _) => {}
            (Some(Block::List), Block::List) => {}
            (Some(Block::Quote), Block::Quote) => out.push_str(">\n"),
            _ => out.push('\n'),
        }
        for (i, line) in body.split('\n').enumerate() {
            let prefix = if i == 0 { &first } else { &rest };
            if line.is_empty() {
                out.push_str(prefix.trim_end());
            } else {
                out.push_str(prefix);
                out.push_str(line);
            }
            out.push('\n');
        }
        previous = Some(block);
    }
    out
}
