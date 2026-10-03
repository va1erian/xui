#![forbid(unsafe_code)]

//! The serde "shadow" types of the JSON format for styles and wraps, with
//! their conversions to and from the model. The model itself derives nothing.

use serde::{Deserialize, Serialize};
use xui_core::backend::TextWeight;
use xui_core::{Color, Dip};

use crate::model::{
    Align, Baseline, BlockKind, CharStyle, LineSpacing, ListItem, ListKind, ParaStyle, Side,
    TextColor, Wrap,
};

type Rgb = [u8; 3];

fn rgb(c: Color) -> Rgb {
    [c.r, c.g, c.b]
}

fn color([r, g, b]: Rgb) -> Color {
    Color::rgb(r, g, b)
}

#[derive(Serialize, Deserialize, Clone, Copy)]
#[serde(rename_all = "snake_case")]
enum BaselineDto {
    Normal,
    Superscript,
    Subscript,
}

#[derive(Serialize, Deserialize)]
pub(super) struct CharDto {
    #[serde(default)]
    family: Option<String>,
    size: f32,
    weight: u16,
    #[serde(default)]
    italic: bool,
    #[serde(default)]
    underline: bool,
    #[serde(default)]
    strike: bool,
    /// `None` follows the theme.
    #[serde(default)]
    color: Option<Rgb>,
    #[serde(default)]
    highlight: Option<Rgb>,
    #[serde(default)]
    link: Option<String>,
    baseline: BaselineDto,
}

impl From<&CharStyle> for CharDto {
    fn from(s: &CharStyle) -> CharDto {
        CharDto {
            family: s.family.clone(),
            size: s.size.0,
            weight: s.weight.value(),
            italic: s.italic,
            underline: s.underline,
            strike: s.strike,
            color: match s.color {
                TextColor::Auto => None,
                TextColor::Fixed(c) => Some(rgb(c)),
            },
            highlight: s.highlight.map(rgb),
            link: s.link.clone(),
            baseline: match s.baseline {
                Baseline::Normal => BaselineDto::Normal,
                Baseline::Superscript => BaselineDto::Superscript,
                Baseline::Subscript => BaselineDto::Subscript,
            },
        }
    }
}

impl From<CharDto> for CharStyle {
    fn from(d: CharDto) -> CharStyle {
        CharStyle {
            family: d.family,
            size: Dip(d.size),
            weight: TextWeight::new(d.weight),
            italic: d.italic,
            underline: d.underline,
            strike: d.strike,
            color: d
                .color
                .map_or(TextColor::Auto, |c| TextColor::Fixed(color(c))),
            highlight: d.highlight.map(color),
            link: d.link,
            baseline: match d.baseline {
                BaselineDto::Normal => Baseline::Normal,
                BaselineDto::Superscript => Baseline::Superscript,
                BaselineDto::Subscript => Baseline::Subscript,
            },
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy)]
#[serde(rename_all = "snake_case")]
enum AlignDto {
    Left,
    Center,
    Right,
    Justify,
}

#[derive(Serialize, Deserialize, Clone, Copy)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum SpacingDto {
    Multiple { value: f32 },
    Exactly { value: f32 },
}

#[derive(Serialize, Deserialize, Clone, Copy)]
#[serde(rename_all = "snake_case")]
enum ListKindDto {
    Bullet,
    Numbered,
}

#[derive(Serialize, Deserialize, Clone, Copy)]
struct ListDto {
    kind: ListKindDto,
    level: u8,
}

#[derive(Serialize, Deserialize, Clone, Copy)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum BlockDto {
    Body,
    Heading { level: u8 },
    Quote,
}

#[derive(Serialize, Deserialize)]
pub(super) struct ParaDto {
    align: AlignDto,
    #[serde(default)]
    indent_left: f32,
    #[serde(default)]
    indent_right: f32,
    #[serde(default)]
    indent_first: f32,
    #[serde(default)]
    space_before: f32,
    #[serde(default)]
    space_after: f32,
    line_spacing: SpacingDto,
    #[serde(default)]
    list: Option<ListDto>,
    block: BlockDto,
}

impl From<&ParaStyle> for ParaDto {
    fn from(s: &ParaStyle) -> ParaDto {
        ParaDto {
            align: match s.align {
                Align::Left => AlignDto::Left,
                Align::Center => AlignDto::Center,
                Align::Right => AlignDto::Right,
                Align::Justify => AlignDto::Justify,
            },
            indent_left: s.indent_left.0,
            indent_right: s.indent_right.0,
            indent_first: s.indent_first.0,
            space_before: s.space_before.0,
            space_after: s.space_after.0,
            line_spacing: match s.line_spacing {
                LineSpacing::Multiple(value) => SpacingDto::Multiple { value },
                LineSpacing::Exactly(d) => SpacingDto::Exactly { value: d.0 },
            },
            list: s.list.map(|item| ListDto {
                kind: match item.kind {
                    ListKind::Bullet => ListKindDto::Bullet,
                    ListKind::Numbered => ListKindDto::Numbered,
                },
                level: item.level,
            }),
            block: match s.kind {
                BlockKind::Body => BlockDto::Body,
                BlockKind::Heading(level) => BlockDto::Heading { level },
                BlockKind::Quote => BlockDto::Quote,
            },
        }
    }
}

impl From<ParaDto> for ParaStyle {
    fn from(d: ParaDto) -> ParaStyle {
        ParaStyle {
            align: match d.align {
                AlignDto::Left => Align::Left,
                AlignDto::Center => Align::Center,
                AlignDto::Right => Align::Right,
                AlignDto::Justify => Align::Justify,
            },
            indent_left: Dip(d.indent_left),
            indent_right: Dip(d.indent_right),
            indent_first: Dip(d.indent_first),
            space_before: Dip(d.space_before),
            space_after: Dip(d.space_after),
            line_spacing: match d.line_spacing {
                SpacingDto::Multiple { value } => LineSpacing::Multiple(value),
                SpacingDto::Exactly { value } => LineSpacing::Exactly(Dip(value)),
            },
            list: d.list.map(|item| ListItem {
                kind: match item.kind {
                    ListKindDto::Bullet => ListKind::Bullet,
                    ListKindDto::Numbered => ListKind::Numbered,
                },
                level: item.level,
            }),
            kind: match d.block {
                BlockDto::Body => BlockKind::Body,
                BlockDto::Heading { level } => BlockKind::Heading(level),
                BlockDto::Quote => BlockKind::Quote,
            },
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy)]
#[serde(rename_all = "snake_case")]
pub(super) enum SideDto {
    Left,
    Right,
}

#[derive(Serialize, Deserialize, Clone, Copy)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(super) enum WrapDto {
    Inline,
    Square { side: SideDto, margin: f32 },
    TopAndBottom { margin: f32 },
}

impl From<Wrap> for WrapDto {
    fn from(wrap: Wrap) -> WrapDto {
        match wrap {
            Wrap::Inline => WrapDto::Inline,
            Wrap::Square { side, margin } => WrapDto::Square {
                side: match side {
                    Side::Left => SideDto::Left,
                    Side::Right => SideDto::Right,
                },
                margin: margin.0,
            },
            Wrap::TopAndBottom { margin } => WrapDto::TopAndBottom { margin: margin.0 },
        }
    }
}

impl From<WrapDto> for Wrap {
    fn from(wrap: WrapDto) -> Wrap {
        match wrap {
            WrapDto::Inline => Wrap::Inline,
            WrapDto::Square { side, margin } => Wrap::Square {
                side: match side {
                    SideDto::Left => Side::Left,
                    SideDto::Right => Side::Right,
                },
                margin: Dip(margin),
            },
            WrapDto::TopAndBottom { margin } => Wrap::TopAndBottom {
                margin: Dip(margin),
            },
        }
    }
}
