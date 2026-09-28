#![forbid(unsafe_code)]

//! The task dialog's geometry: where the card, its icon, text, verification row
//! and buttons go. Kept apart from the widget so the arithmetic is one pure
//! function over the window's client area, DPI and measured text.

use crate::app::Ui;
use crate::backend::{TextStyle, WidgetId};
use crate::geometry::Rect;

use super::{
    BUTTON_HEIGHT, BUTTON_WIDTH, CHECK_HEIGHT, GAP, ICON, MARGIN, MAX_WIDTH, MESSAGE_SIZE,
    MIN_WIDTH, PADDING, TITLE_SIZE, TaskDialogIcon,
};

/// The rectangle each painted part of the card occupies while the dialog is
/// open. The buttons and the checkbox are moved but not painted by the scrim,
/// so only the icon and text need storing.
#[derive(Clone, Copy, Default)]
pub(super) struct Layout {
    /// The card's rectangle.
    pub(super) card: Rect,
    /// The icon's rectangle (meaningful only when `icon_visible`).
    pub(super) icon: Rect,
    /// Whether an icon is drawn.
    pub(super) icon_visible: bool,
    /// The title's rectangle.
    pub(super) title: Rect,
    /// The message's rectangle.
    pub(super) message: Rect,
    /// Whether the card is currently drawn.
    pub(super) visible: bool,
}

/// A computed layout and the moves that place every dialog node.
pub(super) struct Placement {
    pub(super) layout: Layout,
    pub(super) moves: Vec<(WidgetId, Rect)>,
}

/// The button row's nodes: the cancel button and the command buttons in the
/// order they were added.
pub(super) struct Buttons<'a> {
    /// The cancel button, laid out left of every command.
    pub(super) cancel: WidgetId,
    /// The command buttons, in the order the app added them.
    pub(super) commands: &'a [WidgetId],
}

/// Centres the card in the client area and lays out the scrim, the buttons
/// (cancel leftmost, commands to its right, the last command rightmost), the
/// optional verification checkbox and the optional icon.
pub(super) fn place<M: 'static>(
    ui: &Ui<M>,
    scrim: WidgetId,
    buttons: Buttons<'_>,
    verification: Option<WidgetId>,
    icon: TaskDialogIcon,
    title: &str,
    message: &str,
) -> Placement {
    let commands = buttons.commands;
    let cancel = buttons.cancel;
    let dpi = ui.dpi();
    let px = |value: crate::units::Dip| value.to_px(dpi).value();
    let client = ui.client_rect();
    let theme = ui.theme();

    let pad = px(PADDING);
    let gap = px(GAP);
    let button_w = px(BUTTON_WIDTH);
    let button_h = px(BUTTON_HEIGHT);
    let check_h = px(CHECK_HEIGHT);
    let icon_size = px(ICON);
    let margin = px(MARGIN);
    let has_icon = icon != TaskDialogIcon::None;
    let title_metrics = ui.measure_text(title, &TextStyle::new(theme.text, TITLE_SIZE).bold(), dpi);
    let message_metrics = ui.measure_text(
        message,
        &TextStyle::new(theme.text_secondary, MESSAGE_SIZE),
        dpi,
    );

    // Cancel is leftmost; commands keep their order, the last one rightmost.
    let mut row: Vec<WidgetId> = Vec::with_capacity(commands.len() + 1);
    row.push(cancel);
    row.extend_from_slice(commands);

    let text_left = pad + if has_icon { icon_size + gap } else { 0 };
    let avail = (client.width() - margin * 2).max(1);
    let wanted = text_left + title_metrics.width.max(message_metrics.width) + pad;
    let row_wanted = row.len() as i32 * button_w + (row.len() as i32 - 1).max(0) * gap;
    let card_w = wanted
        .max(row_wanted + pad * 2)
        .clamp(px(MIN_WIDTH), px(MAX_WIDTH))
        .min(avail)
        .max(1);

    // A long command row is never wider than the card: shorten the buttons
    // (never lengthen them) so cancel and every command stay inside it.
    let inner_w = (card_w - pad * 2).max(1);
    let gaps = (row.len() as i32 - 1).max(0) * gap;
    let button_w = ((inner_w - gaps) / row.len() as i32).clamp(1, button_w);

    let content_w = (card_w - text_left - pad).max(1);
    let lines = if message_metrics.width > content_w {
        (message_metrics.width + content_w - 1) / content_w
    } else {
        1
    };
    let max_card_h = (client.height() - margin * 2).max(1);
    let wanted_message_h = message_metrics.height.max(1) * lines;
    let heading_h = title_metrics
        .height
        .max(if has_icon { icon_size } else { 0 });
    // Everything but the message; a tall message is shortened (and clipped by
    // the painter) so the card and its command row stay inside the client.
    let chrome_h = pad * 2
        + heading_h
        + gap
        + if verification.is_some() {
            gap + check_h
        } else {
            0
        }
        + gap
        + button_h;
    let message_h = wanted_message_h.min((max_card_h - chrome_h).max(1));
    let card_h = (chrome_h + message_h).min(max_card_h);

    let left = client.left + (client.width() - card_w).max(0) / 2;
    let top = client.top + (client.height() - card_h).max(0) / 2;
    let card = Rect::new(left, top, left + card_w, top + card_h);
    let icon_rect = Rect::new(
        card.left + pad,
        card.top + pad,
        card.left + pad + icon_size,
        card.top + pad + icon_size,
    );
    let title_rect = Rect::new(
        card.left + text_left,
        card.top + pad,
        card.right - pad,
        card.top + pad + title_metrics.height,
    );
    let message_rect = Rect::new(
        card.left + text_left,
        card.top + pad + heading_h + gap,
        card.right - pad,
        card.top + pad + heading_h + gap + message_h,
    );
    let verify_rect = verification.map(|_| {
        Rect::new(
            card.left + pad,
            message_rect.bottom + gap,
            card.right - pad,
            message_rect.bottom + gap + check_h,
        )
    });
    let row_top = (card.bottom - pad - button_h).max(card.top + pad);

    let mut moves = vec![(scrim, client)];
    let count = row.len();
    for (index, button) in row.iter().enumerate() {
        let offset = (count - 1 - index) as i32 * (button_w + gap);
        let right = card.right - pad - offset;
        moves.push((
            *button,
            Rect::new(right - button_w, row_top, right, row_top + button_h),
        ));
    }
    if let (Some(id), Some(rect)) = (verification, verify_rect) {
        moves.push((id, rect));
    }

    Placement {
        layout: Layout {
            card,
            icon: icon_rect,
            icon_visible: has_icon,
            title: title_rect,
            message: message_rect,
            visible: true,
        },
        moves,
    }
}
