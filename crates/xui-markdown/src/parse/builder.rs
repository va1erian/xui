#![forbid(unsafe_code)]

//! The state folded events are built into: one text buffer, blocks and spans.

use std::ops::Range;

use super::slug::Slugs;
use crate::model::{
    Align, Block, BlockKind, Document, ImageRef, Inline, Marker, OBJECT, Span, Table,
};

/// A block whose text is still being appended.
pub(super) struct Open {
    pub kind: BlockKind,
    pub text: u32,
    pub spans: u32,
    /// Trailing newlines are dropped when it closes (HTML blocks).
    pub html: bool,
}

#[derive(Default)]
pub(super) struct Builder {
    pub doc: Document,
    pub open: Option<Open>,
    pub style: Vec<Inline>,
    pub link: Option<u32>,
    /// The image whose alt text is being appended, and where that text
    /// starts.
    pub image: Option<(u32, u32)>,
    /// How many images the parser is in: alt text may hold an image, which
    /// counts as plain alt text of the outer one.
    pub image_depth: u32,
    /// Per enclosing list: the next item number, or `None` for a bullet list.
    pub lists: Vec<Option<u64>>,
    /// A list item's marker, not yet drawn because no block has started in it.
    pub pending: Option<Marker>,
    pub quote: u8,
    /// Spans never merge across this text offset (block or cell start).
    pub floor: u32,
    pub cells: Vec<Range<u32>>,
    pub table: u32,
    pub slugs: Slugs,
}

impl Builder {
    fn pos(&self) -> u32 {
        u32::try_from(self.doc.text.len()).unwrap_or(u32::MAX)
    }

    pub fn style(&self) -> Inline {
        self.style.last().copied().unwrap_or(Inline::NONE)
    }

    /// Starts a block, closing the previous one.
    pub fn open(&mut self, kind: BlockKind, html: bool) {
        self.flush();
        let text = self.pos();
        self.floor = text;
        let spans = u32::try_from(self.doc.spans.len()).unwrap_or(u32::MAX);
        self.open = Some(Open {
            kind,
            text,
            spans,
            html,
        });
    }

    /// Starts a paragraph, which is the item's `ListItem` when it is the first
    /// thing in one.
    pub fn open_paragraph(&mut self) {
        let kind = match self.pending.take() {
            Some(marker) => BlockKind::ListItem(marker),
            None => BlockKind::Paragraph,
        };
        self.open(kind, false);
    }

    /// Before a non-paragraph block: closes the open one and draws a pending
    /// marker as an empty item.
    pub fn begin(&mut self) {
        self.flush();
        if let Some(marker) = self.pending.take() {
            self.open(BlockKind::ListItem(marker), false);
            self.flush();
        }
    }

    /// Appends text with the current style plus `extra`.
    pub fn push(&mut self, text: &str, extra: Inline) {
        if text.is_empty() {
            return;
        }
        if self.open.is_none() {
            self.open_paragraph();
        }
        let start = self.pos();
        self.doc.text.push_str(text);
        let end = self.pos();
        let style = self.style().with(extra);
        let image = self.image.map(|(index, _)| index);
        let first = self.open.as_ref().map_or(0, |o| o.spans as usize);
        if self.doc.spans.len() > first
            && let Some(last) = self.doc.spans.last_mut()
            && last.range.end == start
            && last.range.start >= self.floor
            && last.style == style
            && last.link == self.link
            && last.image == image
        {
            last.range.end = end;
            return;
        }
        let link = self.link;
        self.doc.spans.push(Span {
            range: start..end,
            style,
            link,
            image,
        });
    }

    /// Makes a pending or just-opened plain item a task item.
    pub fn task(&mut self, checked: bool) {
        let marker = Marker::Task(checked);
        match (&mut self.pending, &mut self.open) {
            (Some(pending), _) => *pending = marker,
            (None, Some(open)) if matches!(open.kind, BlockKind::ListItem(_)) => {
                open.kind = BlockKind::ListItem(marker);
            }
            _ => {}
        }
    }

    /// Enters an image; of nested images only the outermost counts.
    pub fn start_image(&mut self, src: &str, title: &str) {
        self.image_depth += 1;
        if self.image_depth > 1 {
            return;
        }
        let index = u32::try_from(self.doc.images.len()).unwrap_or(u32::MAX);
        self.doc.images.push(ImageRef {
            src: src.into(),
            title: title.into(),
        });
        self.image = Some((index, self.pos()));
    }

    /// Leaves an image, giving one with no alt text [`OBJECT`] for it.
    pub fn end_image(&mut self) {
        self.image_depth = self.image_depth.saturating_sub(1);
        if self.image_depth > 0 {
            return;
        }
        if let Some((_, start)) = self.image
            && start == self.pos()
        {
            let mut object = [0; 4];
            self.push(OBJECT.encode_utf8(&mut object), Inline::NONE);
        }
        self.image = None;
    }

    pub fn start_cell(&mut self) {
        self.floor = self.pos();
    }

    pub fn end_cell(&mut self) {
        let start = self.floor;
        self.cells.push(start..self.pos());
    }

    pub fn start_table(&mut self, columns: Vec<Align>) {
        self.begin();
        self.table = self.doc.tables.len() as u32;
        self.doc.tables.push(Table { columns });
    }

    pub fn flush(&mut self) {
        if let Some(open) = self.open.take() {
            self.close(open);
        }
    }

    pub fn finish(mut self) -> Document {
        self.flush();
        self.doc
    }

    fn close(&mut self, mut open: Open) {
        let start = open.text as usize;
        match &mut open.kind {
            BlockKind::Code { .. } => {
                let body = &self.doc.text[start..];
                let body = body.strip_suffix('\n').unwrap_or(body);
                let len = start + body.strip_suffix('\r').unwrap_or(body).len();
                self.truncate(open.spans, len);
            }
            _ if open.html => {
                let len = start + self.doc.text[start..].trim_end_matches(['\n', '\r']).len();
                self.truncate(open.spans, len);
            }
            BlockKind::TableRow { cells, .. } => self.close_row(open.text, open.spans, cells),
            BlockKind::Heading(_) => {
                let slug = self.slugs.next(&self.doc.text[start..]);
                self.doc.anchors.push((slug, self.doc.blocks.len() as u32));
            }
            _ => {}
        }
        let text = open.text..self.pos();
        let spans = open.spans..self.doc.spans.len() as u32;
        self.doc.blocks.push(Block {
            kind: open.kind,
            text,
            spans,
            quote: self.quote,
            list: u8::try_from(self.lists.len()).unwrap_or(u8::MAX),
        });
    }

    /// Fits a row's cells to the table's column count, dropping the text of
    /// extra cells and padding short rows with empty ones.
    fn close_row(&mut self, text: u32, spans: u32, cells: &mut Range<u32>) {
        let columns = self.doc.tables[self.table as usize].columns.len();
        if self.cells.len() > columns {
            self.cells.truncate(columns);
            let len = self.cells.last().map_or(text, |c| c.end);
            self.truncate(spans, len as usize);
        }
        let end = self.pos();
        self.cells.resize(columns, end..end);
        cells.start = self.doc.cells.len() as u32;
        self.doc.cells.append(&mut self.cells);
        cells.end = self.doc.cells.len() as u32;
    }

    /// Cuts the text at `len`, trimming the block's spans (from index `first`).
    fn truncate(&mut self, first: u32, len: usize) {
        self.doc.text.truncate(len);
        while self.doc.spans.len() > first as usize {
            let Some(last) = self.doc.spans.last_mut() else {
                break;
            };
            if last.range.start as usize >= len {
                self.doc.spans.pop();
            } else {
                last.range.end = last.range.end.min(len as u32);
                break;
            }
        }
    }
}
