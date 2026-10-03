//! Shared by the unit tests, the integration tests and the snapshot example:
//! a document builder over the public edit API, and a sample document that
//! exercises the layout.
#![allow(dead_code)]

use std::sync::Arc;

use xui_core::backend::TextWeight;
use xui_core::{Color, Dip, Image};
use xui_rich_text::Document;
use xui_rich_text::model::{
    Align, Baseline, BlockKind, CharStyle, CharStyleId, DocPos, EditOp, InlineImage, ListItem,
    ListKind, ObjectId, ParaStyle, ParaStyleId, ParaStylePatch, Side, TextColor, Wrap,
};

/// An object queued in a [`DocBuilder`], not yet anchored.
#[derive(Clone, Copy, Debug)]
pub struct Obj(usize);

/// One run of a paragraph under construction.
pub enum Run<'a> {
    /// Text in a character style.
    Text(&'a str, CharStyleId),
    /// A queued object, anchored here.
    Object(Obj),
}

/// Assembles a [`Document`] paragraph by paragraph with `Document::apply`.
pub struct DocBuilder {
    doc: Document,
    fresh: bool,
    queued: Vec<(InlineImage, Option<ObjectId>)>,
}

impl Default for DocBuilder {
    fn default() -> DocBuilder {
        DocBuilder::new()
    }
}

/// A patch that sets every attribute of `style`.
fn full_patch(style: &ParaStyle) -> ParaStylePatch {
    ParaStylePatch {
        align: Some(style.align),
        indent_left: Some(style.indent_left),
        indent_right: Some(style.indent_right),
        indent_first: Some(style.indent_first),
        space_before: Some(style.space_before),
        space_after: Some(style.space_after),
        line_spacing: Some(style.line_spacing),
        list: Some(style.list),
        kind: Some(style.kind),
        page_break_before: Some(style.page_break_before),
    }
}

impl DocBuilder {
    /// A builder holding one empty paragraph, which the first
    /// [`paragraph`](DocBuilder::paragraph) fills.
    pub fn new() -> DocBuilder {
        DocBuilder {
            doc: Document::new(),
            fresh: true,
            queued: Vec::new(),
        }
    }

    /// Interns a character style.
    pub fn char_style(&mut self, style: CharStyle) -> CharStyleId {
        self.doc.styles_mut().intern_char(style)
    }

    /// Interns a paragraph style.
    pub fn para_style(&mut self, style: ParaStyle) -> ParaStyleId {
        self.doc.styles_mut().intern_para(style)
    }

    /// Queues an object to anchor with [`Run::Object`].
    pub fn object(&mut self, image: InlineImage) -> Obj {
        self.queued.push((image, None));
        Obj(self.queued.len() - 1)
    }

    /// The id of a queued object, once a paragraph has anchored it.
    pub fn id_of(&self, obj: Obj) -> ObjectId {
        self.queued[obj.0].1.expect("the object is anchored")
    }

    /// Appends a paragraph of `runs` in the paragraph style `style`.
    pub fn paragraph(&mut self, style: ParaStyleId, runs: &[Run<'_>]) -> &mut DocBuilder {
        let index = if self.fresh {
            0
        } else {
            let last = self.doc.paragraph_count() - 1;
            let at = DocPos::new(last, self.doc.paragraphs()[last].text().len());
            self.doc
                .apply(EditOp::SplitParagraph { at })
                .expect("split");
            last + 1
        };
        self.fresh = false;
        let patch = full_patch(&self.doc.styles().para(style).clone());
        self.doc
            .apply(EditOp::SetParaStyle {
                paras: index..index + 1,
                patch,
            })
            .expect("paragraph style");
        for run in runs {
            let at = DocPos::new(index, self.doc.paragraphs()[index].text().len());
            match *run {
                Run::Text("", _) => {}
                Run::Text(text, style) => {
                    let text = text.to_owned();
                    let style = Some(style);
                    self.doc
                        .apply(EditOp::InsertText { at, text, style })
                        .expect("text");
                }
                Run::Object(obj) => {
                    let object = self.queued[obj.0].0.clone();
                    self.doc
                        .apply(EditOp::InsertObject { at, object })
                        .expect("object");
                    let id = self.doc.paragraphs()[index]
                        .objects()
                        .find(|&(byte, _)| byte == at.byte)
                        .map(|(_, id)| id);
                    self.queued[obj.0].1 = id;
                }
            }
        }
        self
    }

    /// The finished document.
    pub fn finish(self) -> Document {
        self.doc
    }
}

/// A picture of `w` by `h` pixels: a vertical gradient from `top` to `bottom`
/// inside a one-pixel dark frame, with a diagonal so scaling shows.
pub fn gradient_image(w: u32, h: u32, top: Color, bottom: Color) -> Arc<Image> {
    let mut pixels = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            let t = y as f32 / h.max(1) as f32;
            let mix = |a: u8, b: u8| (f32::from(a) + (f32::from(b) - f32::from(a)) * t) as u8;
            let edge = x == 0 || y == 0 || x + 1 == w || y + 1 == h;
            let diagonal = x * h / w.max(1) == y;
            let rgb = if edge {
                [40, 40, 48]
            } else if diagonal {
                [255, 255, 255]
            } else {
                [
                    mix(top.r, bottom.r),
                    mix(top.g, bottom.g),
                    mix(top.b, bottom.b),
                ]
            };
            pixels.extend_from_slice(&[rgb[0], rgb[1], rgb[2], 255]);
        }
    }
    Arc::new(Image::from_rgba(w, h, pixels).expect("pixels match the size"))
}

fn picture(image: Arc<Image>, w: f32, h: f32, wrap: Wrap) -> InlineImage {
    InlineImage {
        image,
        size: (Dip(w), Dip(h)),
        wrap,
        alt: String::new(),
    }
}

const PROSE: &str = "Text flows around a floating picture and returns to the full width \
below it, because lines are broken one at a time in whatever room the floats leave free. ";

/// A document with a heading, styled runs, floats on both sides, an inline
/// image, a justified paragraph, a quote and lists.
pub fn sample_document() -> Document {
    let mut b = DocBuilder::new();
    let base = CharStyle::default();
    let plain = CharStyleId::DEFAULT;
    let bold = b.char_style(CharStyle {
        weight: TextWeight::BOLD,
        ..base.clone()
    });
    let italic = b.char_style(CharStyle {
        italic: true,
        ..base.clone()
    });
    let underline = b.char_style(CharStyle {
        underline: true,
        ..base.clone()
    });
    let red = b.char_style(CharStyle {
        color: TextColor::Fixed(Color::rgb(200, 40, 40)),
        ..base.clone()
    });
    let marked = b.char_style(CharStyle {
        highlight: Some(Color::rgb(255, 224, 80)),
        ..base.clone()
    });
    let strike = b.char_style(CharStyle {
        strike: true,
        ..base.clone()
    });
    let big = b.char_style(CharStyle {
        size: Dip(22.0),
        ..base.clone()
    });
    let sup = b.char_style(CharStyle {
        baseline: Baseline::Superscript,
        ..base.clone()
    });
    let sub = b.char_style(CharStyle {
        baseline: Baseline::Subscript,
        ..base.clone()
    });
    let link = b.char_style(CharStyle {
        link: Some("https://example.com".into()),
        ..base
    });

    let body = ParaStyle {
        space_after: Dip(8.0),
        ..ParaStyle::default()
    };
    let body_id = b.para_style(body.clone());
    let heading = b.para_style(ParaStyle {
        kind: BlockKind::Heading(1),
        ..ParaStyle::default()
    });
    let justify = b.para_style(ParaStyle {
        align: Align::Justify,
        ..body.clone()
    });
    let center = b.para_style(ParaStyle {
        align: Align::Center,
        ..body.clone()
    });
    let quote = b.para_style(ParaStyle {
        kind: BlockKind::Quote,
        ..ParaStyle::default()
    });
    let list = |kind, level| ParaStyle {
        list: Some(ListItem { kind, level }),
        space_after: Dip(2.0),
        ..ParaStyle::default()
    };
    let bullet = b.para_style(list(ListKind::Bullet, 0));
    let numbered = b.para_style(list(ListKind::Numbered, 0));
    let nested = b.para_style(list(ListKind::Numbered, 1));

    let left = b.object(picture(
        gradient_image(120, 90, Color::rgb(70, 130, 220), Color::rgb(20, 40, 120)),
        120.0,
        90.0,
        Wrap::square(Side::Left),
    ));
    let right = b.object(picture(
        gradient_image(100, 130, Color::rgb(240, 160, 60), Color::rgb(150, 50, 40)),
        100.0,
        130.0,
        Wrap::square(Side::Right),
    ));
    let inline = b.object(picture(
        gradient_image(48, 32, Color::rgb(90, 190, 120), Color::rgb(20, 90, 60)),
        48.0,
        32.0,
        Wrap::Inline,
    ));

    b.paragraph(heading, &[Run::Text("Rich text layout", plain)]);
    b.paragraph(
        body_id,
        &[
            Run::Object(left),
            Run::Text("Plain, ", plain),
            Run::Text("bold", bold),
            Run::Text(", ", plain),
            Run::Text("italic", italic),
            Run::Text(", ", plain),
            Run::Text("underlined", underline),
            Run::Text(", ", plain),
            Run::Text("struck", strike),
            Run::Text(", ", plain),
            Run::Text("red", red),
            Run::Text(" and ", plain),
            Run::Text("highlighted", marked),
            Run::Text(" runs share lines, with a ", plain),
            Run::Text("larger", big),
            Run::Text(" word, E = mc", plain),
            Run::Text("2", sup),
            Run::Text(", H", plain),
            Run::Text("2", sub),
            Run::Text("O and a ", plain),
            Run::Text("link", link),
            Run::Text(". ", plain),
            Run::Text(PROSE, plain),
            Run::Text(PROSE, plain),
        ],
    );
    b.paragraph(
        body_id,
        &[
            Run::Object(right),
            Run::Text(PROSE, plain),
            Run::Text("A picture on the right pushes the line ends in. ", italic),
            Run::Text(PROSE, plain),
        ],
    );
    b.paragraph(
        body_id,
        &[
            Run::Text("An inline image ", plain),
            Run::Object(inline),
            Run::Text(
                " sits on the baseline like a large glyph, and the line grows to hold it. ",
                plain,
            ),
            Run::Text(PROSE, plain),
        ],
    );
    b.paragraph(
        justify,
        &[
            Run::Text("This paragraph is justified: ", bold),
            Run::Text(PROSE, plain),
            Run::Text(PROSE, plain),
        ],
    );
    b.paragraph(center, &[Run::Text("A centred line", italic)]);
    b.paragraph(
        quote,
        &[Run::Text(
            "A quotation is indented and ruled on the left.",
            italic,
        )],
    );
    b.paragraph(bullet, &[Run::Text("A bullet", plain)]);
    b.paragraph(
        bullet,
        &[Run::Text(
            "Another bullet, long enough to wrap onto a second line so the hanging indent shows",
            plain,
        )],
    );
    b.paragraph(numbered, &[Run::Text("First step", plain)]);
    b.paragraph(nested, &[Run::Text("A nested step", plain)]);
    b.paragraph(numbered, &[Run::Text("Second step", plain)]);
    b.finish()
}
