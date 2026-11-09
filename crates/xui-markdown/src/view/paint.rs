#![forbid(unsafe_code)]

//! Painting the visible blocks from their cached layouts. Drawing a laid-out
//! block shapes and allocates nothing; `State::prepare` first lays out any
//! visible block that has no layout yet (the only shaping a paint does).

use std::ops::Range;

use xui_core::backend::{Canvas, Rgba};
use xui_core::geometry::{Point, Rect};
use xui_core::widget::scrollbar::{self, Orientation, ThumbState};
use xui_core::{Color, Theme};

use super::state::State;
use crate::layout::{BlockLayout, Decor, FRect, Layout, overlaps};
use crate::style::{self, CodePalette, Role};

/// Paints the document and the scrollbar.
///
/// The software compositor reports [`Canvas::bounds`] as the node's window
/// rectangle with no translation applied, while pointer events arrive
/// node-local; so the origin moves to the node's corner and everything below
/// works in node-local coordinates.
pub(crate) fn paint(canvas: &mut dyn Canvas, state: &mut State, theme: &Theme) {
    let origin = canvas.bounds();
    let bounds = Rect::from_size(origin.size());
    canvas.save();
    canvas.set_translation(origin.left as f32, origin.top as f32);
    paint_local(canvas, state, theme, bounds);
    canvas.restore();
}

fn paint_local(canvas: &mut dyn Canvas, state: &mut State, theme: &Theme, bounds: Rect) {
    state.prepare(bounds, canvas.dpi());
    canvas.fill_rect(bounds, style::background(theme));
    let text_area = Rect::new(bounds.left, bounds.top, state.track.left, bounds.bottom);
    canvas.push_clip(text_area);
    let y0 = state.scroll - state.pad;
    let range = state.layout.visible(y0, y0 + state.viewport());
    let palette = state.palette;
    let ink = Ink {
        theme,
        palette: palette.as_ref(),
        thin: (canvas.dpi() as f32 / 96.0).round().max(1.0),
        selection: if state.focused {
            theme.selection
        } else {
            theme.selection_unfocused
        },
    };
    let selection = state.selection.range();
    for index in range {
        let top = state.layout.top(index);
        if !selection.is_empty() {
            state.rects.prepare(&state.layout, &selection, index);
        }
        if let Some(laid) = state.layout.block(index) {
            let at = (state.pad, top - y0);
            let selected = if selection.is_empty() {
                &[][..]
            } else {
                state.rects.get(index)
            };
            let images = Images {
                layout: &state.layout,
                selection: &selection,
            };
            block(canvas, laid, at, &ink, selected, &images);
        }
    }
    canvas.pop_clip();
    let scroll = state.bar_scroll();
    let thumb = if state.bar_drag.is_some() {
        ThumbState::Pressed
    } else {
        ThumbState::Normal
    };
    scrollbar::paint_state(
        canvas,
        state.track,
        scroll,
        Orientation::Vertical,
        *theme,
        thumb,
    );
}

/// `r` moved by `(dx, dy)`, in whole pixels, at least one pixel each way.
fn pixels(r: &FRect, (dx, dy): (f32, f32)) -> Rect {
    let left = (r.x + dx).round() as i32;
    let top = (r.y + dy).round() as i32;
    let right = ((r.x + dx + r.w).round() as i32).max(left + 1);
    let bottom = ((r.y + dy + r.h).round() as i32).max(top + 1);
    Rect::new(left, top, right, bottom)
}

fn draw(canvas: &mut dyn Canvas, decor: &Decor, at: (f32, f32), ink: &Ink) {
    let color = |role: &Role| role.color(ink.theme, ink.palette);
    match decor {
        Decor::Fill(r, role) => canvas.fill_rect(pixels(r, at), color(role)),
        Decor::Frame(r, role) => canvas.stroke_rect(pixels(r, at), color(role), ink.thin),
        Decor::Disc(r, role) => {
            let rect = pixels(r, at);
            let radius = rect.width().min(rect.height()) as f32 / 2.0;
            canvas.fill_rounded_rect(rect, radius, color(role));
        }
    }
}

/// The colours and stroke a paint uses.
struct Ink<'a> {
    theme: &'a Theme,
    palette: Option<&'a CodePalette>,
    thin: f32,
    selection: Color,
}

/// Where a block's images come from, and what is selected.
struct Images<'a> {
    layout: &'a Layout,
    selection: &'a Range<u32>,
}

fn block(
    canvas: &mut dyn Canvas,
    laid: &BlockLayout,
    at: (f32, f32),
    ink: &Ink,
    selected: &[FRect],
    images: &Images,
) {
    let (theme, palette) = (ink.theme, ink.palette);
    // Backgrounds (code and header shading) go under the selection; lines,
    // frames and markers over it, so a selected link keeps its underline.
    let background = |d: &&Decor| matches!(d, Decor::Fill(_, Role::Surface));
    for decor in laid.decor.iter().filter(background) {
        draw(canvas, decor, at, ink);
    }
    // A code block clips its long lines, and their selection with them.
    if let Some(clip) = &laid.clip {
        canvas.push_clip(pixels(clip, at));
    }
    for rect in selected {
        canvas.fill_rect(pixels(rect, at), ink.selection);
    }
    if laid.clip.is_some() {
        canvas.pop_clip();
    }
    for decor in laid.decor.iter().filter(|d| !background(d)) {
        draw(canvas, decor, at, ink);
    }
    for image in &laid.images {
        let Some(bitmap) = images.layout.image(image.image) else {
            continue;
        };
        let rect = pixels(&image.rect, at);
        canvas.draw_image(bitmap, rect);
        // The selection band is under the image: tint a selected one.
        if overlaps(&image.range, images.selection) {
            let tint = Rgba::from(ink.selection);
            canvas.fill_rect_rgba(rect, Rgba { a: 128, ..tint });
        }
    }
    if let Some(clip) = &laid.clip {
        canvas.push_clip(pixels(clip, at));
    }
    for piece in &laid.pieces {
        let origin = Point::new(
            (piece.x + at.0).round() as i32,
            (piece.y + at.1).round() as i32,
        );
        canvas.draw_layout(
            piece.layout.as_ref(),
            origin,
            Rgba::from(piece.role.color(theme, palette)),
        );
    }
    if laid.clip.is_some() {
        canvas.pop_clip();
    }
}
