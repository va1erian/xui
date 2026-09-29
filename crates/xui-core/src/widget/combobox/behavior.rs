#![forbid(unsafe_code)]

//! [`ComboBox`](super::ComboBox)'s popup geometry, painting and event handling,
//! split from `combobox.rs` so both files stay under the size limit.

use crate::app::Ui;
use crate::backend::{Canvas, Event, TextStyle, WidgetId};
use crate::geometry::{Point, Rect};
use crate::icon::draw_icon;
use crate::message::{Key, MouseButton};
use crate::theme::Theme;
use crate::widget::popup;

use super::{ARROW, ICON, ICON_GAP, PADDING, ROW, Shared, TEXT_SIZE};

fn pick<T>(cond: bool, yes: T, no: T) -> T {
    if cond { yes } else { no }
}

/// The popup's rectangle: directly below `field`, one row per item.
pub(super) fn popup_rect(field: Rect, dpi: u32, count: usize) -> Rect {
    let row = ROW.to_px(dpi).value().max(1);
    let bottom = field.bottom + row * count as i32;
    Rect::new(field.left, field.bottom, field.right, bottom)
}

/// Shows or hides the popup to match `want`, moving it under the field first.
fn set_open<M: 'static>(
    ui: &Ui<M>,
    (field, popup): (WidgetId, WidgetId),
    s: &Shared<M>,
    want: bool,
) {
    if want == s.open.replace(want) {
        return;
    }
    if want {
        ui.apply_moves(&[(popup, popup_rect(ui.bounds(field), ui.dpi(), s.items.len()))]);
    }
    ui.set_visible(popup, want);
    if want {
        // The popup is a sibling created before the widgets below it, so raise
        // it above them or the list draws behind them.
        ui.raise(popup);
    }
    ui.invalidate(field);
    ui.invalidate(popup);
}

/// The row an event `y` (node-local) falls on.
fn row_at(dpi: u32, y: i32, count: usize) -> Option<usize> {
    (y >= 0)
        .then(|| (y / ROW.to_px(dpi).value().max(1)) as usize)
        .filter(|index| *index < count)
}

/// Draws item `index`'s icon, if it has one, at the left of `text` and returns
/// `text` moved right past it (unchanged for an item without an icon).
fn lead<M: 'static>(
    s: &Shared<M>,
    canvas: &mut dyn Canvas,
    index: usize,
    text: Rect,
    color: crate::color::Color,
) -> Rect {
    let Some(icon) = s.icons.borrow().get(index).copied().flatten() else {
        return text;
    };
    let dpi = canvas.dpi();
    let side = ICON.to_px(dpi).value();
    let top = text.top + (text.height() - side) / 2;
    let slot = Rect::new(text.left, top, text.left + side, top + side);
    draw_icon(canvas, icon, slot, color, dpi);
    let shift = side + ICON_GAP.to_px(dpi).value();
    Rect::new(
        (text.left + shift).min(text.right),
        text.top,
        text.right,
        text.bottom,
    )
}

/// Paints the field: input background, border, selected text and a chevron.
pub(super) fn paint_field<M: 'static>(
    s: &Shared<M>,
    canvas: &mut dyn Canvas,
    theme: Theme,
    flag: bool,
) {
    let b = canvas.bounds();
    let dpi = canvas.dpi();
    let (pad, arrow) = (PADDING.to_px(dpi).value(), ARROW.to_px(dpi).value());
    canvas.clear(theme.input_background);
    let color = pick(s.enabled.get(), theme.text, theme.text_disabled);
    let border = pick(s.open.get(), theme.border_focused, theme.input_border);
    canvas.stroke_rect(b, border, 1.0);
    let i = b.shrink(pad);
    let text = Rect::new(i.left, i.top, (i.right - arrow).max(i.left), i.bottom);
    if let Some(item) = s.items.get(s.selected.get()) {
        let text = lead(s, canvas, s.selected.get(), text, color);
        canvas.draw_text(item, text, &TextStyle::new(color, TEXT_SIZE).middle());
    }
    let (cx, cy) = (b.right - pad - arrow / 2, b.top + b.height() / 2);
    let half = (arrow / 2).max(2);
    let p1 = Point::new(cx - half, cy - half / 2);
    let p2 = Point::new(cx, cy + half / 2);
    let p3 = Point::new(cx + half, cy - half / 2);
    canvas.draw_line(p1, p2, color, 1.5);
    canvas.draw_line(p2, p3, color, 1.5);
    if flag {
        canvas.stroke_rect(b, theme.accent, 2.0);
    }
}

/// Paints every popup row, highlighting the selected and hovered ones.
pub(super) fn paint_popup<M: 'static>(s: &Shared<M>, canvas: &mut dyn Canvas, theme: Theme) {
    let b = canvas.bounds();
    let row = ROW.to_px(canvas.dpi()).value().max(1);
    let pad = PADDING.to_px(canvas.dpi()).value();
    popup::paint(canvas, theme, theme.surface);
    for (index, item) in s.items.iter().enumerate() {
        let top = b.top + row * index as i32;
        let rect = Rect::new(b.left, top, b.right, top + row);
        let hot = index == s.selected.get() || s.hover.get() == Some(index);
        if hot {
            canvas.fill_rect(rect, theme.accent);
        }
        let hot_color = pick(hot, theme.text_on_accent, theme.text);
        let color = pick(s.enabled.get(), hot_color, theme.text_disabled);
        let style = TextStyle::new(color, TEXT_SIZE).middle();
        let text = lead(s, canvas, index, rect.shrink(pad), color);
        canvas.draw_text(item, text, &style);
    }
}

/// Handles field input: click and Space/Return toggle, Escape closes, arrows
/// move the selection.
pub(super) fn field_event<M: 'static>(
    s: &Shared<M>,
    ui: &Ui<M>,
    field: WidgetId,
    popup: WidgetId,
    event: &Event,
) -> Option<M> {
    if (ui.is_design_mode() && event.is_input()) || !s.enabled.get() {
        return None;
    }
    let len = s.items.len();
    let toggle = !s.open.get();
    match event {
        Event::MouseDown { button, .. } if *button == MouseButton::Left => {
            set_open(ui, (field, popup), s, toggle);
        }
        Event::KeyDown { key, .. } => match *key {
            Key::RETURN | Key::SPACE => set_open(ui, (field, popup), s, toggle),
            Key::ESCAPE => set_open(ui, (field, popup), s, false),
            Key::UP | Key::DOWN => {
                let cur = s.selected.get();
                let max = len.saturating_sub(1);
                s.selected.set(match *key {
                    Key::DOWN => (cur + 1).min(max),
                    _ => cur.saturating_sub(1),
                });
                ui.invalidate(field);
                ui.invalidate(popup);
            }
            _ => {}
        },
        _ => {}
    }
    None
}

/// Handles popup input: hover highlights, a left click chooses a row.
pub(super) fn popup_event<M: 'static>(
    s: &Shared<M>,
    ui: &Ui<M>,
    field: WidgetId,
    popup: WidgetId,
    event: &Event,
) -> Option<M> {
    if (ui.is_design_mode() && event.is_input()) || !s.enabled.get() {
        return None;
    }
    if let Event::MouseMove { y, .. } = event {
        let hover = row_at(ui.dpi(), *y, s.items.len());
        if s.hover.get() != hover {
            s.hover.set(hover);
            ui.invalidate(popup);
        }
        return None;
    }
    let Event::MouseDown { y, button, .. } = event else {
        return None;
    };
    if *button != MouseButton::Left {
        return None;
    }
    let index = row_at(ui.dpi(), *y, s.items.len())?;
    s.selected.set(index);
    set_open(ui, (field, popup), s, false);
    let mapper = s.on_select.borrow();
    mapper.as_ref().and_then(|mapper| mapper(index))
}
