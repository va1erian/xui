#![forbid(unsafe_code)]

//! The native, lossless save format: versioned JSON.
//!
//! ```json
//! {"version": 1,
//!  "styles": {"chars": [...], "paras": [...]},
//!  "objects": [{"id": 0, "png_base64": "...", "width": 120, "height": 80,
//!               "wrap": {"kind": "inline"}, "alt": ""}],
//!  "paragraphs": [{"text": "...", "spans": [[len, style], ...],
//!                  "anchors": [0], "style": 0}]}
//! ```
//!
//! Styles are written as the interned tables, so ids survive a round trip.
//! Images are embedded as base64 PNG (a JPEG source comes back as PNG). The
//! file types are serde "shadow" structs; the model derives nothing.

mod dto;

use std::collections::HashSet;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use xui_core::{Dip, Image};

use super::FormatError;
use super::base64;
use crate::model::{
    CharStyle, CharStyleId, Document, InlineImage, ObjectId, ObjectTable, ParaStyle, ParaStyleId,
    Paragraph, Span, StyleTable,
};
use dto::{CharDto, ParaDto, WrapDto};

/// The version `to_json` writes and `from_json` reads.
const VERSION: u32 = 1;

#[derive(Serialize, Deserialize)]
struct Probe {
    version: u32,
}

#[derive(Serialize, Deserialize)]
struct FileDto {
    version: u32,
    styles: StylesDto,
    objects: Vec<ObjectDto>,
    paragraphs: Vec<ParagraphDto>,
}

#[derive(Serialize, Deserialize)]
struct StylesDto {
    chars: Vec<CharDto>,
    paras: Vec<ParaDto>,
}

#[derive(Serialize, Deserialize)]
struct ObjectDto {
    id: u32,
    png_base64: String,
    width: f32,
    height: f32,
    wrap: WrapDto,
    #[serde(default)]
    alt: String,
}

#[derive(Serialize, Deserialize)]
struct ParagraphDto {
    text: String,
    spans: Vec<(usize, u32)>,
    #[serde(default)]
    anchors: Vec<u32>,
    style: u32,
}

/// Writes `doc` as JSON.
///
/// Only objects some paragraph anchors are written. An image that cannot be
/// re-encoded (impossible for one built through `Image`'s constructors) is
/// written empty and fails to load.
pub fn to_json(doc: &Document) -> String {
    let mut ids: Vec<ObjectId> = doc
        .paragraphs()
        .iter()
        .flat_map(|p| p.anchors().iter().copied())
        .collect();
    ids.sort();
    ids.dedup();
    let objects = ids
        .into_iter()
        .filter_map(|id| doc.objects().get(id).map(|object| (id, object)))
        .map(|(id, object)| ObjectDto {
            id: id.0,
            png_base64: base64::encode(&object.image.encode_png().unwrap_or_default()),
            width: object.size.0.0,
            height: object.size.1.0,
            wrap: object.wrap.into(),
            alt: object.alt.clone(),
        })
        .collect();
    let file = FileDto {
        version: VERSION,
        styles: StylesDto {
            chars: doc.styles().chars().iter().map(CharDto::from).collect(),
            paras: doc.styles().paras().iter().map(ParaDto::from).collect(),
        },
        objects,
        paragraphs: doc
            .paragraphs()
            .iter()
            .map(|p| ParagraphDto {
                text: p.text().to_owned(),
                spans: p.spans().iter().map(|s| (s.len, s.style.0)).collect(),
                anchors: p.anchors().iter().map(|id| id.0).collect(),
                style: p.style().0,
            })
            .collect(),
    };
    serde_json::to_string(&file).expect("the shadow types always serialize")
}

/// Reads a document written by [`to_json`], validating every invariant.
pub fn from_json(json: &str) -> Result<Document, FormatError> {
    let probe: Probe =
        serde_json::from_str(json).map_err(|e| FormatError::Syntax(e.to_string()))?;
    if probe.version != VERSION {
        return Err(FormatError::Version(probe.version));
    }
    let file: FileDto =
        serde_json::from_str(json).map_err(|e| FormatError::Syntax(e.to_string()))?;

    let styles = build_styles(file.styles)?;
    let mut objects = ObjectTable::new();
    for dto in file.objects {
        let (id, object) = build_object(dto)?;
        if objects.get(id).is_some() {
            return Err(FormatError::Invalid(format!("duplicate object {}", id.0)));
        }
        objects.insert_with_id(id, object);
    }
    let paragraphs = file
        .paragraphs
        .into_iter()
        .enumerate()
        .map(|(i, p)| build_paragraph(i, p))
        .collect::<Result<Vec<_>, _>>()?;
    let mut anchored = HashSet::new();
    for id in paragraphs.iter().flat_map(|p| p.anchors()) {
        if !anchored.insert(*id) {
            return Err(FormatError::Invalid(format!(
                "object {} anchored twice",
                id.0
            )));
        }
    }
    Document::from_parts(paragraphs, styles, objects).map_err(FormatError::Invalid)
}

fn build_styles(dto: StylesDto) -> Result<StyleTable, FormatError> {
    let mut table = StyleTable::new();
    for (i, style) in dto.chars.into_iter().map(CharStyle::from).enumerate() {
        if !style.size.0.is_finite() || style.size.0 <= 0.0 {
            return Err(FormatError::Invalid(format!(
                "character style {i}: bad size"
            )));
        }
        if table.intern_char(style).0 as usize != i {
            return Err(FormatError::Invalid(format!(
                "character style {i} duplicates an earlier one or is not the default first"
            )));
        }
    }
    for (i, style) in dto.paras.into_iter().map(ParaStyle::from).enumerate() {
        if let crate::model::BlockKind::Heading(level) = style.kind
            && !(1..=3).contains(&level)
        {
            return Err(FormatError::Invalid(format!(
                "paragraph style {i}: bad heading level"
            )));
        }
        if table.intern_para(style).0 as usize != i {
            return Err(FormatError::Invalid(format!(
                "paragraph style {i} duplicates an earlier one or is not the default first"
            )));
        }
    }
    Ok(table)
}

fn build_object(dto: ObjectDto) -> Result<(ObjectId, InlineImage), FormatError> {
    let bytes = base64::decode(&dto.png_base64)
        .ok_or_else(|| FormatError::Image(format!("object {}: bad base64", dto.id)))?;
    let image =
        Image::decode(&bytes).map_err(|e| FormatError::Image(format!("object {}: {e}", dto.id)))?;
    let valid = |v: f32| v.is_finite() && v >= 0.0;
    if !valid(dto.width) || !valid(dto.height) {
        return Err(FormatError::Invalid(format!("object {}: bad size", dto.id)));
    }
    let object = InlineImage {
        image: Arc::new(image),
        size: (Dip(dto.width), Dip(dto.height)),
        wrap: dto.wrap.into(),
        alt: dto.alt,
    };
    Ok((ObjectId(dto.id), object))
}

fn build_paragraph(index: usize, dto: ParagraphDto) -> Result<Paragraph, FormatError> {
    let covered = dto
        .spans
        .iter()
        .try_fold(0usize, |sum, &(len, _)| sum.checked_add(len));
    if covered != Some(dto.text.len()) {
        return Err(FormatError::Invalid(format!(
            "paragraph {index}: spans do not cover the text"
        )));
    }
    let mut paragraph = Paragraph::new(String::new(), ParaStyleId(dto.style), CharStyleId::DEFAULT);
    paragraph.text = dto.text;
    paragraph.spans = dto
        .spans
        .into_iter()
        .map(|(len, style)| Span {
            len,
            style: CharStyleId(style),
        })
        .collect();
    paragraph.anchors = dto.anchors.into_iter().map(ObjectId).collect();
    Ok(paragraph)
}
