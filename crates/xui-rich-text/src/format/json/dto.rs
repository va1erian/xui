#![forbid(unsafe_code)]

//! The serde "shadow" types of the JSON format for styles and wraps, with
//! their conversions to and from the model. The model itself derives nothing.

use serde::{Deserialize, Serialize};
use xui_core::backend::TextWeight;
use xui_core::{Color, Dip};

use crate::model::{
    Align, Baseline, BlockKind, CellMark, CellStart, CharStyle, LineSpacing, ListItem, ListKind,
    PageSetup, ParaStyle, Side, Table, TableId, TextColor, Wrap,
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
    #[serde(default, skip_serializing_if = "is_false")]
    page_break_before: bool,
}

fn is_false(value: &bool) -> bool {
    !*value
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
            page_break_before: s.page_break_before,
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
            page_break_before: d.page_break_before,
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

/// The page setup, in dip.
#[derive(Serialize, Deserialize)]
pub(super) struct PageDto {
    width: f32,
    height: f32,
    margins: [f32; 4],
}

impl From<&PageSetup> for PageDto {
    fn from(p: &PageSetup) -> PageDto {
        PageDto {
            width: p.width.0,
            height: p.height.0,
            margins: [p.left.0, p.top.0, p.right.0, p.bottom.0],
        }
    }
}

impl From<PageDto> for PageSetup {
    fn from(d: PageDto) -> PageSetup {
        let [left, top, right, bottom] = d.margins.map(Dip);
        PageSetup {
            width: Dip(d.width),
            height: Dip(d.height),
            left,
            top,
            right,
            bottom,
        }
    }
}

/// A table's settings.
#[derive(Serialize, Deserialize)]
pub(super) struct TableDto {
    id: u32,
    columns: Vec<f32>,
    #[serde(default)]
    header: bool,
    #[serde(default = "yes")]
    border: bool,
}

fn yes() -> bool {
    true
}

impl TableDto {
    pub(super) fn new(id: u32, t: &Table) -> TableDto {
        TableDto {
            id,
            columns: t.columns.clone(),
            header: t.header,
            border: t.border,
        }
    }

    pub(super) fn id(&self) -> u32 {
        self.id
    }
}

impl From<TableDto> for Table {
    fn from(d: TableDto) -> Table {
        Table {
            columns: d.columns,
            header: d.header,
            border: d.border,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy)]
#[serde(rename_all = "snake_case")]
enum CellStartDto {
    Row,
    Cell,
    Continue,
}

/// A paragraph's place in a table.
#[derive(Serialize, Deserialize, Clone, Copy)]
pub(super) struct CellDto {
    table: u32,
    start: CellStartDto,
}

impl From<CellMark> for CellDto {
    fn from(c: CellMark) -> CellDto {
        CellDto {
            table: c.table.0,
            start: match c.start {
                CellStart::Row => CellStartDto::Row,
                CellStart::Cell => CellStartDto::Cell,
                CellStart::Continue => CellStartDto::Continue,
            },
        }
    }
}

impl From<CellDto> for CellMark {
    fn from(d: CellDto) -> CellMark {
        CellMark {
            table: TableId(d.table),
            start: match d.start {
                CellStartDto::Row => CellStart::Row,
                CellStartDto::Cell => CellStart::Cell,
                CellStartDto::Continue => CellStart::Continue,
            },
        }
    }
}
