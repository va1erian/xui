#![forbid(unsafe_code)]

//! Page view: where the sheets go, and painting them on the desk.

use xui_core::backend::{Canvas, Rgba};
use xui_core::geometry::{Point, Rect};
use xui_core::{Color, Dip, Theme};

use super::state::State;
use crate::layout::Pages;

/// The desk around and between the sheets in page view, at 100%.
const DESK_GAP: Dip = Dip(16.0);
/// The smallest zoom page view shrinks a sheet to, to fit the window.
const MIN_ZOOM: f32 = 0.25;

/// Where the sheets of page view are, in client pixels before scrolling.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Sheets {
    /// The sheet's left edge.
    pub left: i32,
    /// The first sheet's top.
    pub top: f32,
    /// The sheet's width.
    pub width: f32,
    /// The sheet's height.
    pub height: f32,
    /// From one sheet's top to the next one's.
    pub pitch: f32,
    /// The text area's offset from the sheet's left edge.
    pub margin_left: f32,
    /// The text area's offset from the sheet's top.
    pub margin_top: f32,
}

impl State {
    /// Page view: lays the flow out in the page's text area, at a lower DPI
    /// when a sheet is wider than the view, and centres the sheets.
    pub(crate) fn prepare_sheets(&mut self, text_area: Rect, dpi: u32) {
        let page = *self.ed.doc.page();
        let gap = DESK_GAP.0 * dpi as f32 / 96.0;
        let full = page.width.0 * dpi as f32 / 96.0;
        let room = text_area.width() as f32 - 2.0 * gap;
        let zoom = (room / full).clamp(MIN_ZOOM, 1.0);
        // The zoom is a DPI, so text is shaped at the size it is shown.
        let layout_dpi = ((dpi as f32 * zoom).round() as u32).max(1);
        let px = |d: Dip| d.0 * layout_dpi as f32 / 96.0;
        let gap = DESK_GAP.0 * layout_dpi as f32 / 96.0;
        let (width, height) = (px(page.width), px(page.height));
        let pitch = height + gap;
        self.layout
            .set_metrics(px(page.content_width()).max(1.0), layout_dpi);
        self.layout
            .set_pages(Pages::new(px(page.content_height()), pitch));
        let left =
            text_area.left + ((text_area.width() as f32 - width) / 2.0).max(gap).round() as i32;
        let sheets = Sheets {
            left,
            top: gap,
            width,
            height,
            pitch,
            margin_left: px(page.left),
            margin_top: px(page.top),
        };
        self.origin = Point::new(
            left + sheets.margin_left.round() as i32,
            (sheets.top + sheets.margin_top).round() as i32,
        );
        self.sheets = Some(sheets);
    }
}

/// The desk the sheets lie on: a step darker than the window in a light
/// theme, the window's own background in a dark one.
pub(crate) fn desk(theme: &Theme) -> Color {
    if theme.is_dark {
        theme.background
    } else {
        let c = theme.background;
        let darker = |v: u8| (f32::from(v) * 0.88).round() as u8;
        Color::rgb(darker(c.r), darker(c.g), darker(c.b))
    }
}

/// Paints the visible sheets of page view: a soft shadow, the white paper
/// and a hairline edge.
pub(crate) fn paint(canvas: &mut dyn Canvas, state: &State, sheets: Sheets, theme: &Theme) {
    let scroll = state.scroll.round();
    let count = state.layout.page_count();
    let first = (((scroll - sheets.top) / sheets.pitch).floor().max(0.0)) as usize;
    let shadow = (2.0 * state.dpi as f32 / 96.0).round().max(1.0) as i32;
    let paper = Theme::light().input_background;
    let edge = if theme.is_dark {
        Rgba::with_alpha(0, 0, 0, 160)
    } else {
        Rgba::with_alpha(0, 0, 0, 48)
    };
    for index in first..count {
        let top = sheets.top + index as f32 * sheets.pitch - scroll;
        if top > state.viewport {
            break;
        }
        let rect = Rect::new(
            sheets.left,
            top.round() as i32,
            sheets.left + sheets.width.round() as i32,
            (top + sheets.height).round() as i32,
        );
        canvas.fill_rect_rgba(rect.offset(shadow, shadow), Rgba::with_alpha(0, 0, 0, 40));
        canvas.fill_rect(rect, paper);
        canvas.fill_rect_rgba(
            Rect::new(rect.left, rect.top, rect.right, rect.top + 1),
            edge,
        );
        canvas.fill_rect_rgba(
            Rect::new(rect.left, rect.bottom - 1, rect.right, rect.bottom),
            edge,
        );
        canvas.fill_rect_rgba(
            Rect::new(rect.left, rect.top, rect.left + 1, rect.bottom),
            edge,
        );
        canvas.fill_rect_rgba(
            Rect::new(rect.right - 1, rect.top, rect.right, rect.bottom),
            edge,
        );
    }
}
