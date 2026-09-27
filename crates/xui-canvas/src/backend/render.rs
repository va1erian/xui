#![forbid(unsafe_code)]

//! Compositing a window's nodes into a [`Surface`].

use std::rc::Rc;

use xui_core::Theme;
use xui_core::backend::{Canvas as _, WidgetId, WindowId};
use xui_core::geometry::Rect;
use xui_core::image::Image;

use super::Shared;
use super::geometry::{absolute_bounds, ancestor_clip, effectively_visible, intersect};
use crate::Surface;
use crate::gl::GlWidget;

/// Where a window's GL content is attached.
#[derive(Clone, Copy)]
enum GlSlot {
    /// Covers the whole client area, as the base layer.
    Window,
    /// Covers one node's bounds, composited among the window's other nodes.
    Node(WidgetId),
}

/// A node to draw, in creation order: its painter (if any) and whether it also
/// carries GL content.
struct NodeDraw {
    id: WidgetId,
    bounds: Rect,
    clip: Option<Rect>,
    painter: Option<xui_core::backend::Painter>,
    gl: bool,
}

/// Fills `surface` with the window's background, composites the window's
/// window-level GL content as the base layer, then runs each visible node's
/// painter at its absolute bounds, in creation order (later nodes on top),
/// clipped to its ancestors' clips, with per-node GL content at that node's
/// bounds.
///
/// GL content is one painter among many: each frame is rendered into a texture,
/// read back as pixels and drawn through the same software surface, so CPU nodes
/// composite over it instead of being hidden by a GPU frame that took over the
/// whole window; node-level content sits in its pane.
pub(super) fn composite(shared: &Shared, window: WindowId, surface: &mut Surface) {
    let (theme, dpi, size) = {
        let windows = shared.windows.borrow();
        let Some(state) = windows.get(&window.raw()) else {
            return;
        };
        (state.theme, state.dpi, state.size)
    };
    surface.fill(theme.background);
    draw_gl(
        shared,
        window,
        GlSlot::Window,
        Rect::new(0, 0, size.0 as i32, size.1 as i32),
        None,
        theme,
        dpi,
        surface,
    );

    let draws: Vec<NodeDraw> = {
        let nodes = shared.nodes.borrow();
        let windows = shared.windows.borrow();
        let gl_nodes = windows.get(&window.raw()).map(|state| &state.gl_nodes);
        nodes
            .iter()
            .filter(|(id, node)| node.window == window && effectively_visible(&nodes, *id))
            .filter_map(|(id, node)| {
                let painter = node.painter.clone();
                let gl = gl_nodes.is_some_and(|nodes| nodes.contains_key(id));
                if painter.is_none() && !gl {
                    return None;
                }
                let bounds = absolute_bounds(&nodes, *id)?;
                let clip = ancestor_clip(&nodes, *id);
                if let Some(clip) = clip
                    && intersect(bounds, clip).is_empty()
                {
                    return None;
                }
                Some(NodeDraw {
                    id: *id,
                    bounds,
                    clip,
                    painter,
                    gl,
                })
            })
            .collect()
    };
    for draw in draws {
        if let Some(painter) = &draw.painter {
            surface.with_canvas_at(draw.bounds, dpi, |canvas| {
                if let Some(clip) = draw.clip {
                    canvas.push_clip(clip);
                }
                painter(canvas);
            });
        }
        if draw.gl {
            draw_gl(
                shared,
                window,
                GlSlot::Node(draw.id),
                draw.bounds,
                draw.clip,
                theme,
                dpi,
                surface,
            );
        }
    }
}

/// Renders the GL content at `slot` into an image of `bounds`' size and draws
/// it at `bounds`. When no GL context or framebuffer is available the widget's
/// software fallback is painted instead, so the window stays usable.
#[allow(clippy::too_many_arguments)]
fn draw_gl(
    shared: &Shared,
    window: WindowId,
    slot: GlSlot,
    bounds: Rect,
    clip: Option<Rect>,
    theme: Theme,
    dpi: u32,
    surface: &mut Surface,
) {
    if bounds.is_empty() {
        return;
    }
    let Some(widget) = gl_widget(shared, window, slot) else {
        return;
    };
    let image = render_image(shared, window, &widget, bounds, theme);
    surface.with_canvas_at(bounds, dpi, |canvas| {
        if let Some(clip) = clip {
            canvas.push_clip(clip);
        }
        match &image {
            Some(image) => canvas.draw_image(image, bounds),
            None => widget.paint(canvas, bounds, &theme),
        }
    });
}

/// The GL widget attached at `slot`, if any.
fn gl_widget(shared: &Shared, window: WindowId, slot: GlSlot) -> Option<Rc<dyn GlWidget>> {
    let windows = shared.windows.borrow();
    let state = windows.get(&window.raw())?;
    match slot {
        GlSlot::Window => state.gl.clone(),
        GlSlot::Node(id) => state.gl_nodes.get(&id).cloned(),
    }
}

/// Renders `widget` into an offscreen image of `bounds`' size, or `None` when no
/// GL context or framebuffer is available.
fn render_image(
    shared: &Shared,
    window: WindowId,
    widget: &Rc<dyn GlWidget>,
    bounds: Rect,
    theme: Theme,
) -> Option<Image> {
    let mut windows = shared.windows.borrow_mut();
    let state = windows.get_mut(&window.raw())?;
    let handle = state.window.clone()?;
    state.renderer.render(
        &handle,
        bounds.width() as u32,
        bounds.height() as u32,
        theme.background,
        |gl| widget.paint_gl(gl, bounds, &theme),
        |gl| widget.gl_teardown(gl),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::Node;
    use xui_core::backend::{ParentRef, WidgetId, WindowId};

    fn node(window: WindowId, parent: ParentRef, bounds: Rect) -> Node {
        Node {
            window,
            parent,
            bounds,
            visible: true,
            enabled: true,
            text: String::new(),
            painter: None,
            drag_region: false,
            clip: None,
        }
    }

    #[test]
    fn nested_offsets_accumulate_to_window_coordinates() {
        let window = WindowId::from_raw(1);
        let panel = WidgetId::from_raw(10);
        let child = WidgetId::from_raw(11);
        let nodes = vec![
            (
                panel,
                node(
                    window,
                    ParentRef::Window(window),
                    Rect::new(100, 50, 200, 90),
                ),
            ),
            (
                child,
                node(window, ParentRef::Widget(panel), Rect::new(5, 7, 25, 17)),
            ),
        ];

        assert_eq!(
            absolute_bounds(&nodes, panel),
            Some(Rect::new(100, 50, 200, 90))
        );
        assert_eq!(
            absolute_bounds(&nodes, child),
            Some(Rect::new(105, 57, 125, 67))
        );
    }

    #[test]
    fn an_ancestors_clip_bounds_a_childs_painted_area() {
        let window = WindowId::from_raw(1);
        let view = WidgetId::from_raw(20);
        let child = WidgetId::from_raw(21);
        let mut view_node = node(window, ParentRef::Window(window), Rect::new(0, 0, 100, 80));
        // The view clips its descendants to a 10px band at its top.
        view_node.clip = Some(Rect::new(0, 0, 100, 10));
        let nodes = vec![
            (view, view_node),
            (
                child,
                node(window, ParentRef::Widget(view), Rect::new(0, 5, 100, 40)),
            ),
        ];

        let clip = ancestor_clip(&nodes, child).expect("the child is clipped");
        let bounds = absolute_bounds(&nodes, child).unwrap();
        assert_eq!(
            intersect(bounds, clip),
            Rect::new(0, 5, 100, 10),
            "the child's bounds are chopped to the view's clip"
        );
    }

    #[test]
    fn a_child_outside_its_ancestors_clip_is_culled() {
        let window = WindowId::from_raw(1);
        let view = WidgetId::from_raw(30);
        let child = WidgetId::from_raw(31);
        let mut view_node = node(window, ParentRef::Window(window), Rect::new(0, 0, 100, 80));
        view_node.clip = Some(Rect::new(0, 0, 100, 10));
        let nodes = vec![
            (view, view_node),
            (
                child,
                node(window, ParentRef::Widget(view), Rect::new(0, 20, 100, 40)),
            ),
        ];

        let clip = ancestor_clip(&nodes, child).expect("the child is clipped");
        let bounds = absolute_bounds(&nodes, child).unwrap();
        assert!(
            intersect(bounds, clip).is_empty(),
            "a child entirely below the clip is culled"
        );
    }
}
