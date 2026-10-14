#![forbid(unsafe_code)]

//! The compositor both canvas backends share: what to paint, in which order,
//! and how, for a window of a [`Scene`].
//!
//! A frame is two steps so that no borrow of the scene is held while painters
//! run (a painter may call back into the backend):
//!
//! 1. [`plan`] walks the window's nodes once, in creation order, and keeps the
//!    ones that paint: visible through their whole ancestry, with something
//!    left inside their ancestors' clips. Each carries its window-absolute
//!    bounds, its clip, and the rectangle its pixels can reach.
//! 2. [`Plan::paint`] repaints rectangles of a [`Surface`]: for each, the
//!    window background, then every planned node whose reach meets it, in
//!    creation order, clipped to its ancestors' clips.
//!
//! Painting a rectangle is a [`Surface::paint_region`], so the pixels inside it
//! are exactly what a whole-window paint draws there and the rest are not
//! touched. The windowed backend and the offscreen backend call the same two
//! functions, so what they draw cannot drift.
//!
//! GL content (the windowed backend's renderer, the offscreen backend's
//! software fallback) is a [`Content`] the backend lends the compositor.

use std::collections::HashMap;

use xui_core::Theme;
use xui_core::backend::{Canvas as _, Painter, ParentRef, WidgetId, WindowId};
use xui_core::geometry::Rect;

use crate::geometry::{Placement, intersect};
use crate::scene::Scene;
use crate::{Region, Surface};

/// How far a painter may draw outside its node's bounds, in pixels.
///
/// Strokes are centred on a shape's edge, so a 1 px outline on the bounds'
/// edge covers half a pixel outside it and anti-aliasing shows it; a focus
/// ring or a glow (a theme's accent halo reaches about three pixels) goes
/// further. The compositor repaints, besides every node a damaged rectangle
/// touches, every node whose bounds *grown by this margin* touch it, and
/// damage is grown by it too, so the overdraw of a neighbour is redrawn and
/// the overdraw of a changed node is not left behind. A painter that draws
/// further out than this has to report it with
/// [`Backend::invalidate_rect`](xui_core::backend::Backend::invalidate_rect),
/// whose rectangle is honoured as given.
pub(crate) const OVERDRAW: i32 = 8;

/// `rect` grown by `by` pixels on every side; an empty rectangle stays empty.
pub(crate) fn grow(rect: Rect, by: i32) -> Rect {
    if rect.is_empty() {
        return rect;
    }
    Rect::new(
        rect.left - by,
        rect.top - by,
        rect.right + by,
        rect.bottom + by,
    )
}

/// What the compositor lends a backend's extra content: GPU-rendered widgets,
/// or their software fallback. Everything it paints goes through the same
/// region as ordinary nodes.
pub(crate) trait Content {
    /// Whether node `id` carries content of its own, so it is planned even
    /// without a painter.
    fn has_node(&self, id: WidgetId) -> bool;

    /// Whether the window's content changes without the compositor being told
    /// (a GL frame; a software fallback reading a live value). A live window
    /// is repainted whole every frame.
    fn is_live(&self) -> bool;

    /// Paints the window-level content as the base layer, over `window`.
    fn paint_window(&self, region: &mut Region<'_>, window: Rect, dpi: u32, theme: &Theme);

    /// Paints node `id`'s content at its absolute `bounds`, under `clip`.
    fn paint_node(
        &self,
        region: &mut Region<'_>,
        id: WidgetId,
        bounds: Rect,
        clip: Option<Rect>,
        dpi: u32,
        theme: &Theme,
    );
}

/// One node to draw.
struct Draw {
    id: WidgetId,
    /// Window-absolute bounds.
    bounds: Rect,
    /// The clip its ancestors impose, in window coordinates.
    clip: Option<Rect>,
    /// Where its pixels can land: its bounds trimmed to the clip, grown by
    /// [`OVERDRAW`].
    reach: Rect,
    painter: Option<Painter>,
    /// Whether `Content` also paints at this node.
    content: bool,
}

/// The nodes of one window that paint, in creation order.
pub(crate) struct Plan {
    draws: Vec<Draw>,
}

/// Resolves `id`'s placement, and its ancestors' on the way, into `resolved`.
/// `chain` is scratch space for the walk up.
fn place(
    scene: &Scene,
    id: WidgetId,
    resolved: &mut HashMap<WidgetId, Placement>,
    chain: &mut Vec<WidgetId>,
) -> Option<Placement> {
    if let Some(placement) = resolved.get(&id) {
        return Some(*placement);
    }
    chain.clear();
    let mut above: Option<Placement> = None;
    let mut current = id;
    loop {
        let node = scene.get(current)?;
        chain.push(current);
        match node.parent {
            ParentRef::Window(_) => break,
            ParentRef::Widget(parent) => {
                if let Some(placement) = resolved.get(&parent) {
                    above = Some(*placement);
                    break;
                }
                current = parent;
            }
        }
    }
    for member in chain.iter().rev() {
        let placement = Placement::of(scene.get(*member)?, above.as_ref());
        resolved.insert(*member, placement);
        above = Some(placement);
    }
    above
}

/// Plans a frame of `window`: its painting nodes in creation order, with
/// bounds and clips resolved in a single pass over the scene.
pub(crate) fn plan(scene: &Scene, window: WindowId, content: &dyn Content) -> Plan {
    let mut resolved = HashMap::new();
    let mut chain = Vec::new();
    let mut draws = Vec::new();
    for (id, node) in scene.iter() {
        if node.window != window {
            continue;
        }
        let has_content = content.has_node(id);
        if node.painter.is_none() && !has_content {
            continue;
        }
        let Some(placement) = place(scene, id, &mut resolved, &mut chain) else {
            continue;
        };
        if !placement.visible {
            continue;
        }
        let visible = match placement.clip {
            Some(clip) => intersect(placement.bounds, clip),
            None => placement.bounds,
        };
        if placement.clip.is_some() && visible.is_empty() {
            continue;
        }
        draws.push(Draw {
            id,
            bounds: placement.bounds,
            clip: placement.clip,
            reach: grow(visible, OVERDRAW),
            painter: node.painter.clone(),
            content: has_content,
        });
    }
    Plan { draws }
}

/// What painting a frame needs besides the plan.
pub(crate) struct FrameInfo<'a> {
    /// The window's rectangle, `(0, 0, width, height)`.
    pub(crate) window: Rect,
    /// Dots per inch the painters scale their design values by.
    pub(crate) dpi: u32,
    /// The theme the background reads.
    pub(crate) theme: &'a Theme,
}

impl Plan {
    /// How many nodes the plan paints.
    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.draws.len()
    }

    /// The ids of the planned nodes, in paint order.
    #[cfg(test)]
    pub(crate) fn ids(&self) -> Vec<WidgetId> {
        self.draws.iter().map(|draw| draw.id).collect()
    }

    /// Repaints each of `areas` (window coordinates, inside the window) of
    /// `surface`: the window background, then the planned nodes whose reach
    /// meets the area, in creation order.
    ///
    /// Every pixel inside an area ends up as a whole-window paint draws it;
    /// pixels outside are not touched.
    pub(crate) fn paint(
        &self,
        surface: &mut Surface,
        areas: &[Rect],
        frame: &FrameInfo<'_>,
        content: &dyn Content,
    ) {
        for area in areas {
            surface.paint_region(*area, |region| {
                region.with_canvas_at(frame.window, frame.dpi, |canvas| {
                    xui_core::theme::look::paint_background(
                        canvas,
                        *area,
                        frame.window,
                        frame.theme,
                    );
                });
                content.paint_window(region, frame.window, frame.dpi, frame.theme);
                for draw in &self.draws {
                    if intersect(draw.reach, *area).is_empty() {
                        continue;
                    }
                    // Nodes paint in creation order, so a widget's container
                    // is already on the surface when the widget paints over it.
                    if let Some(painter) = &draw.painter {
                        region.with_canvas_over_parents(draw.bounds, frame.dpi, |canvas| {
                            if let Some(clip) = draw.clip {
                                canvas.push_clip(clip);
                            }
                            painter(canvas);
                        });
                    }
                    if draw.content {
                        content.paint_node(
                            region,
                            draw.id,
                            draw.bounds,
                            draw.clip,
                            frame.dpi,
                            frame.theme,
                        );
                    }
                }
            });
        }
    }
}

#[cfg(test)]
mod tests;
