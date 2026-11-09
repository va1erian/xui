#![forbid(unsafe_code)]

//! Pulldown-cmark events folded into the builder.

use pulldown_cmark::{Alignment, CodeBlockKind, Event, HeadingLevel, Tag, TagEnd};

use super::builder::Builder;
use crate::model::{Align, BlockKind, Inline, Marker};

impl Builder {
    pub fn event(&mut self, event: Event<'_>) {
        match event {
            Event::Start(tag) => self.start(tag),
            Event::End(tag) => self.end(tag),
            Event::Text(text) | Event::Html(text) | Event::InlineHtml(text) => {
                self.push(&text, Inline::NONE)
            }
            Event::Code(code) => self.push(&code, Inline::CODE),
            Event::SoftBreak => self.push(" ", Inline::NONE),
            Event::HardBreak => self.push("\n", Inline::NONE),
            Event::Rule => {
                self.begin();
                self.open(BlockKind::Rule, false);
                self.flush();
            }
            Event::TaskListMarker(checked) => self.task(checked),
            _ => {}
        }
    }

    fn start(&mut self, tag: Tag<'_>) {
        match tag {
            Tag::Paragraph => self.open_paragraph(),
            Tag::Heading { level, .. } => {
                self.begin();
                self.open(BlockKind::Heading(heading_level(level)), false);
            }
            Tag::BlockQuote(_) => {
                self.begin();
                self.quote = self.quote.saturating_add(1);
            }
            Tag::CodeBlock(kind) => {
                self.begin();
                let lang = match &kind {
                    CodeBlockKind::Fenced(info) => info.split_whitespace().next().unwrap_or(""),
                    CodeBlockKind::Indented => "",
                };
                self.open(BlockKind::Code { lang: lang.into() }, false);
            }
            Tag::HtmlBlock => {
                self.begin();
                self.open(BlockKind::Paragraph, true);
            }
            Tag::List(start) => {
                self.begin();
                self.lists.push(start);
            }
            Tag::Item => {
                self.flush();
                self.pending = Some(match self.lists.last_mut() {
                    Some(Some(n)) => {
                        let marker = Marker::Number(*n);
                        *n = n.saturating_add(1);
                        marker
                    }
                    _ => Marker::Bullet,
                });
            }
            Tag::Table(aligns) => self.start_table(aligns.into_iter().map(align).collect()),
            Tag::TableHead => self.start_row(true),
            Tag::TableRow => self.start_row(false),
            Tag::TableCell => self.start_cell(),
            Tag::Emphasis => self.nest(Inline::ITALIC),
            Tag::Strong => self.nest(Inline::BOLD),
            Tag::Strikethrough => self.nest(Inline::STRIKE),
            Tag::Image {
                dest_url, title, ..
            } => {
                self.nest(Inline::IMAGE);
                self.start_image(&dest_url, &title);
            }
            Tag::Link { dest_url, .. } => {
                self.link = Some(self.doc.links.len() as u32);
                self.doc.links.push(dest_url.as_ref().into());
            }
            _ => {}
        }
    }

    fn end(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::Paragraph
            | TagEnd::Heading(_)
            | TagEnd::CodeBlock
            | TagEnd::HtmlBlock
            | TagEnd::TableHead
            | TagEnd::TableRow => self.flush(),
            TagEnd::BlockQuote(_) => {
                self.flush();
                self.quote = self.quote.saturating_sub(1);
            }
            TagEnd::List(_) => {
                self.flush();
                self.lists.pop();
            }
            TagEnd::Item => self.begin(),
            TagEnd::TableCell => self.end_cell(),
            TagEnd::Emphasis | TagEnd::Strong | TagEnd::Strikethrough => {
                self.style.pop();
            }
            TagEnd::Image => {
                self.end_image();
                self.style.pop();
            }
            TagEnd::Link => self.link = None,
            _ => {}
        }
    }

    fn nest(&mut self, flag: Inline) {
        let style = self.style().with(flag);
        self.style.push(style);
    }

    fn start_row(&mut self, header: bool) {
        self.begin();
        let kind = BlockKind::TableRow {
            table: self.table,
            header,
            cells: 0..0,
        };
        self.open(kind, false);
    }
}

fn heading_level(level: HeadingLevel) -> u8 {
    level as u8
}

fn align(a: Alignment) -> Align {
    match a {
        Alignment::None => Align::None,
        Alignment::Left => Align::Left,
        Alignment::Center => Align::Center,
        Alignment::Right => Align::Right,
    }
}
