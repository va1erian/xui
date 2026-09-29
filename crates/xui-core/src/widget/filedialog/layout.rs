#![forbid(unsafe_code)]

//! The file dialog's geometry: where the card, its text and its controls go.
//!
//! Kept apart from the widget so the arithmetic is one pure function over the
//! window's client area, its DPI and a few design constants.

use crate::app::Ui;
use crate::backend::WidgetId;
use crate::geometry::Rect;
use crate::units::Dip;

/// Padding between the card edge and its content.
const PADDING: Dip = Dip(12.0);
/// Vertical gap between the card's blocks.
const GAP: Dip = Dip(8.0);
/// The title row's height.
const TITLE_HEIGHT: Dip = Dip(22.0);
/// The path and filename fields' height.
const FIELD_HEIGHT: Dip = Dip(26.0);
/// The inline error row's height.
const ERROR_HEIGHT: Dip = Dip(18.0);
/// The button size.
const BUTTON_WIDTH: Dip = Dip(86.0);
const BUTTON_HEIGHT: Dip = Dip(28.0);
/// The filter selector's width.
const FILTER_WIDTH: Dip = Dip(120.0);
/// The card's smallest and largest sides, and its margin from the window edge.
const MIN_WIDTH: Dip = Dip(380.0);
const MAX_WIDTH: Dip = Dip(680.0);
const MIN_HEIGHT: Dip = Dip(280.0);
const MAX_HEIGHT: Dip = Dip(480.0);
const MARGIN: Dip = Dip(24.0);
/// The card's corner radius, in pixels.
pub(super) const RADIUS: f32 = 8.0;
/// The title's text size.
pub(super) const TITLE_SIZE: Dip = Dip(15.0);

/// The rectangles the dialog paints or places while it is open.
#[derive(Clone, Copy, Default)]
pub(super) struct Layout {
    pub(super) card: Rect,
    pub(super) title: Rect,
    pub(super) visible: bool,
}

/// A computed layout and the moves that place every node.
pub(super) struct Placement {
    pub(super) layout: Layout,
    pub(super) moves: Vec<(WidgetId, Rect)>,
}

/// The nodes the card arranges. Kept as one value so the argument list stays
/// short and the call sites read like the layout they describe.
pub(super) struct Parts {
    pub(super) scrim: WidgetId,
    pub(super) path: WidgetId,
    pub(super) list: WidgetId,
    pub(super) name: WidgetId,
    pub(super) filter: Option<WidgetId>,
    pub(super) error: WidgetId,
    pub(super) accept: WidgetId,
    pub(super) cancel: WidgetId,
}

/// Centres the card in the client area and lays out the path bar, the entry
/// list, the filename field, an optional filter selector, the error row and the
/// accept/cancel buttons.
pub(super) fn place<M: 'static>(ui: &Ui<M>, parts: &Parts) -> Placement {
    let dpi = ui.dpi();
    let px = |value: Dip| value.to_px(dpi).value();
    let client = ui.client_rect();

    let pad = px(PADDING);
    let gap = px(GAP);
    let title_h = px(TITLE_HEIGHT);
    let field_h = px(FIELD_HEIGHT);
    let error_h = px(ERROR_HEIGHT);
    let button_w = px(BUTTON_WIDTH);
    let button_h = px(BUTTON_HEIGHT);
    let filter_w = px(FILTER_WIDTH);
    let margin = px(MARGIN);

    let avail_w = (client.width() - margin * 2).max(1);
    let avail_h = (client.height() - margin * 2).max(1);
    let card_w = avail_w.min(px(MAX_WIDTH)).max(px(MIN_WIDTH).min(avail_w));
    let card_h = avail_h.min(px(MAX_HEIGHT)).max(px(MIN_HEIGHT).min(avail_h));
    let left = client.left + (client.width() - card_w).max(0) / 2;
    let top = client.top + (client.height() - card_h).max(0) / 2;
    let card = Rect::new(left, top, left + card_w, top + card_h);
    let inner_left = card.left + pad;
    let inner_right = card.right - pad;

    let title_rect = Rect::new(
        inner_left,
        card.top + pad,
        inner_right,
        card.top + pad + title_h,
    );
    let path_top = title_rect.bottom + gap;
    let path_rect = Rect::new(inner_left, path_top, inner_right, path_top + field_h);

    let name_bottom = card.bottom - pad - button_h - gap - error_h - gap;
    let name_rect = Rect::new(inner_left, name_bottom - field_h, inner_right, name_bottom);
    let error_rect = Rect::new(
        inner_left,
        name_rect.bottom + gap,
        inner_right,
        name_bottom + gap + error_h,
    );
    let button_top = card.bottom - pad - button_h;

    let list_top = path_rect.bottom + gap;
    let list_bottom = (name_rect.top - gap).max(list_top + 1);
    let list_rect = Rect::new(inner_left, list_top, inner_right, list_bottom);

    let mut moves = vec![
        (parts.scrim, client),
        (parts.path, path_rect),
        (parts.list, list_rect),
        (parts.error, error_rect),
        (
            parts.cancel,
            Rect::new(
                card.right - pad - button_w,
                button_top,
                card.right - pad,
                button_top + button_h,
            ),
        ),
        (
            parts.accept,
            Rect::new(
                card.right - pad - button_w * 2 - gap,
                button_top,
                card.right - pad - button_w - gap,
                button_top + button_h,
            ),
        ),
    ];
    let name_right = match parts.filter {
        Some(filter) => {
            moves.push((
                filter,
                Rect::new(
                    inner_right - filter_w,
                    name_rect.top,
                    inner_right,
                    name_rect.bottom,
                ),
            ));
            inner_right - filter_w - gap
        }
        None => inner_right,
    };
    moves.push((
        parts.name,
        Rect::new(name_rect.left, name_rect.top, name_right, name_rect.bottom),
    ));

    Placement {
        layout: Layout {
            card,
            title: title_rect,
            visible: true,
        },
        moves,
    }
}
