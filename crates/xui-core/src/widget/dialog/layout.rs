#![forbid(unsafe_code)]

//! The dialog's geometry: where the card, its text and its controls go.
//!
//! Kept apart from the widget so the layout arithmetic is one pure function
//! over the window's client area, DPI and measured text.

use crate::app::Ui;
use crate::backend::{TextStyle, WidgetId};
use crate::geometry::Rect;

use super::{
    BUTTON_HEIGHT, BUTTON_WIDTH, FIELD_HEIGHT, GAP, MARGIN, MAX_WIDTH, MESSAGE_SIZE, MIN_WIDTH,
    PADDING, TITLE_SIZE,
};

/// The rectangle each part of the card occupies while the dialog is open.
#[derive(Clone, Copy, Default)]
pub(super) struct Layout {
    pub(super) card: Rect,
    pub(super) title: Rect,
    pub(super) message: Rect,
    pub(super) visible: bool,
}

/// A computed layout and the moves that place every dialog node.
pub(super) struct Placement {
    pub(super) layout: Layout,
    pub(super) moves: Vec<(WidgetId, Rect)>,
}

/// Centres the card in the client area and lays out the scrim, the buttons
/// (right-aligned, last one rightmost) and an optional prompt field.
pub(super) fn place<M: 'static>(
    ui: &Ui<M>,
    scrim: WidgetId,
    buttons: &[WidgetId],
    field: Option<WidgetId>,
    title: &str,
    message: &str,
) -> Placement {
    let dpi = ui.dpi();
    let px = |value: crate::units::Dip| value.to_px(dpi).value();
    let client = ui.client_rect();
    let theme = ui.theme();

    let pad = px(PADDING);
    let gap = px(GAP);
    let button_w = px(BUTTON_WIDTH);
    let button_h = px(BUTTON_HEIGHT);
    let field_h = px(FIELD_HEIGHT);
    let margin = px(MARGIN);
    let title_metrics = ui.measure_text(title, &TextStyle::new(theme.text, TITLE_SIZE).bold(), dpi);
    let message_metrics = ui.measure_text(message, &TextStyle::new(theme.text, MESSAGE_SIZE), dpi);

    let avail = (client.width() - margin * 2).max(px(MIN_WIDTH));
    let wanted = title_metrics.width.max(message_metrics.width) + pad * 2;
    let card_w = wanted.clamp(px(MIN_WIDTH), px(MAX_WIDTH)).min(avail);
    let content_w = (card_w - pad * 2).max(1);
    let lines = if message_metrics.width > content_w {
        (message_metrics.width + content_w - 1) / content_w
    } else {
        1
    };
    let message_h = message_metrics.height.max(1) * lines;
    let mut card_h = pad * 2 + title_metrics.height + gap + message_h;
    if field.is_some() {
        card_h += gap + field_h;
    }
    card_h += gap + button_h;

    let left = client.left + (client.width() - card_w).max(0) / 2;
    let top = client.top + (client.height() - card_h).max(0) / 2;
    let card = Rect::new(left, top, left + card_w, top + card_h);
    let title_rect = Rect::new(
        card.left + pad,
        card.top + pad,
        card.right - pad,
        card.top + pad + title_metrics.height,
    );
    let message_rect = Rect::new(
        card.left + pad,
        title_rect.bottom + gap,
        card.right - pad,
        title_rect.bottom + gap + message_h,
    );
    let field_rect = field.map(|_| {
        Rect::new(
            card.left + pad,
            message_rect.bottom + gap,
            card.right - pad,
            message_rect.bottom + gap + field_h,
        )
    });
    let row_top = card.bottom - pad - button_h;

    let mut moves = vec![(scrim, client)];
    let count = buttons.len();
    for (index, button) in buttons.iter().enumerate() {
        let offset = (count - 1 - index) as i32 * (button_w + gap);
        let right = card.right - pad - offset;
        moves.push((
            *button,
            Rect::new(right - button_w, row_top, right, row_top + button_h),
        ));
    }
    if let (Some(field), Some(rect)) = (field, field_rect) {
        moves.push((field, rect));
    }

    Placement {
        layout: Layout {
            card,
            title: title_rect,
            message: message_rect,
            visible: true,
        },
        moves,
    }
}
