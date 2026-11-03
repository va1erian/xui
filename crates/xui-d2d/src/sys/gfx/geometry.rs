//! Path geometries: rounded rectangles with per-corner radii, polygons and
//! xui paths, plus the layer clip built from one.

use std::mem::ManuallyDrop;

use windows::Win32::Graphics::Direct2D::Common::{
    D2D_RECT_F, D2D_SIZE_F, D2D1_BEZIER_SEGMENT, D2D1_FIGURE_BEGIN_FILLED, D2D1_FIGURE_END_CLOSED,
    D2D1_FIGURE_END_OPEN,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1_ANTIALIAS_MODE_PER_PRIMITIVE, D2D1_ARC_SEGMENT, D2D1_ARC_SIZE_SMALL,
    D2D1_LAYER_OPTIONS_NONE, D2D1_LAYER_PARAMETERS, D2D1_SWEEP_DIRECTION_CLOCKWISE, ID2D1Geometry,
    ID2D1GeometrySink,
};
use windows::core::Interface;
use windows_numerics::{Matrix3x2, Vector2};
use xui_core::backend::{Corner, PathPlacement, PathSeg};

use super::target::Target;
use super::{RectF, factory};

/// A reusable Direct2D path geometry.
pub(crate) struct Geometry(ID2D1Geometry);

fn point(x: f32, y: f32) -> Vector2 {
    Vector2 { X: x, Y: y }
}

impl Geometry {
    pub(super) fn raw(&self) -> &ID2D1Geometry {
        &self.0
    }

    /// Builds a geometry by describing figures to a sink.
    fn build(describe: impl FnOnce(&ID2D1GeometrySink)) -> Option<Geometry> {
        factory::with(|f| {
            // SAFETY: a geometry and its sink, closed before the geometry is
            // used; the sink is only touched inside `describe`.
            unsafe {
                let path = f.d2d.CreatePathGeometry().ok()?;
                let sink = path.Open().ok()?;
                describe(&sink);
                sink.Close().ok()?;
                path.cast::<ID2D1Geometry>().ok().map(Geometry)
            }
        })?
    }

    /// A rectangle whose corners (top-left, top-right, bottom-right,
    /// bottom-left) carry their own elliptical radii.
    pub(crate) fn rounded_rect(rect: RectF, corners: [Corner; 4]) -> Option<Geometry> {
        let (half_w, half_h) = (rect.width() / 2.0, rect.height() / 2.0);
        let [tl, tr, br, bl] = corners.map(|c| (c.x.clamp(0.0, half_w), c.y.clamp(0.0, half_h)));
        Geometry::build(|sink| {
            // SAFETY: figure calls on an open sink, each figure ended once.
            unsafe {
                sink.BeginFigure(point(rect.left + tl.0, rect.top), D2D1_FIGURE_BEGIN_FILLED);
                sink.AddLine(point(rect.right - tr.0, rect.top));
                arc(sink, rect.right, rect.top + tr.1, tr);
                sink.AddLine(point(rect.right, rect.bottom - br.1));
                arc(sink, rect.right - br.0, rect.bottom, br);
                sink.AddLine(point(rect.left + bl.0, rect.bottom));
                arc(sink, rect.left, rect.bottom - bl.1, bl);
                sink.AddLine(point(rect.left, rect.top + tl.1));
                arc(sink, rect.left + tl.0, rect.top, tl);
                sink.EndFigure(D2D1_FIGURE_END_CLOSED);
            }
        })
    }

    /// The closed polygon through `points` (at least three).
    pub(crate) fn polygon(points: &[(f32, f32)]) -> Option<Geometry> {
        let (first, rest) = points.split_first()?;
        Geometry::build(|sink| {
            // SAFETY: one closed figure on an open sink.
            unsafe {
                sink.BeginFigure(point(first.0, first.1), D2D1_FIGURE_BEGIN_FILLED);
                for p in rest {
                    sink.AddLine(point(p.0, p.1));
                }
                sink.EndFigure(D2D1_FIGURE_END_CLOSED);
            }
        })
    }

    /// An xui path, placed by `at`.
    pub(crate) fn path(segs: &[PathSeg], at: PathPlacement) -> Option<Geometry> {
        Geometry::build(|sink| {
            let mut open = false;
            let mut pen = (0.0, 0.0);
            // SAFETY: figures are begun before segments are added and ended
            // exactly once, on an open sink.
            unsafe {
                for seg in segs {
                    match *seg {
                        PathSeg::MoveTo(x, y) => {
                            if open {
                                sink.EndFigure(D2D1_FIGURE_END_OPEN);
                            }
                            pen = at.apply(x, y);
                            sink.BeginFigure(point(pen.0, pen.1), D2D1_FIGURE_BEGIN_FILLED);
                            open = true;
                        }
                        PathSeg::LineTo(x, y) => {
                            begin_if_needed(sink, &mut open, pen);
                            pen = at.apply(x, y);
                            sink.AddLine(point(pen.0, pen.1));
                        }
                        PathSeg::CubicTo(x1, y1, x2, y2, x, y) => {
                            begin_if_needed(sink, &mut open, pen);
                            let (c1, c2) = (at.apply(x1, y1), at.apply(x2, y2));
                            pen = at.apply(x, y);
                            sink.AddBezier(&D2D1_BEZIER_SEGMENT {
                                point1: point(c1.0, c1.1),
                                point2: point(c2.0, c2.1),
                                point3: point(pen.0, pen.1),
                            });
                        }
                        PathSeg::Close => {
                            if open {
                                sink.EndFigure(D2D1_FIGURE_END_CLOSED);
                                open = false;
                            }
                        }
                    }
                }
                if open {
                    sink.EndFigure(D2D1_FIGURE_END_OPEN);
                }
            }
        })
    }
}

/// # Safety
/// `sink` must be open and not inside a figure when `open` is false.
unsafe fn begin_if_needed(sink: &ID2D1GeometrySink, open: &mut bool, pen: (f32, f32)) {
    if !*open {
        // SAFETY: the caller guarantees an open sink outside a figure.
        unsafe { sink.BeginFigure(point(pen.0, pen.1), D2D1_FIGURE_BEGIN_FILLED) };
        *open = true;
    }
}

/// # Safety
/// `sink` must be open and inside a figure.
unsafe fn arc(sink: &ID2D1GeometrySink, x: f32, y: f32, radii: (f32, f32)) {
    if radii.0 <= 0.0 || radii.1 <= 0.0 {
        // SAFETY: the caller guarantees a figure is open.
        unsafe { sink.AddLine(point(x, y)) };
        return;
    }
    // SAFETY: the caller guarantees a figure is open.
    unsafe {
        sink.AddArc(&D2D1_ARC_SEGMENT {
            point: point(x, y),
            size: D2D_SIZE_F {
                width: radii.0,
                height: radii.1,
            },
            rotationAngle: 0.0,
            sweepDirection: D2D1_SWEEP_DIRECTION_CLOCKWISE,
            arcSize: D2D1_ARC_SIZE_SMALL,
        })
    };
}

impl Target {
    /// Clips to `geometry` (anti-aliased) until [`Target::pop_layer`].
    pub(crate) fn push_clip_geometry(&self, geometry: &Geometry) {
        // SAFETY: inside a frame. The layer outlives its push (it is kept on
        // `layers` until the pop); the mask field is a borrowed reference held
        // through `ManuallyDrop` and released explicitly after the call, which
        // is how the C-style struct expects its COM pointer to be handled.
        unsafe {
            let Ok(layer) = self.rt.CreateLayer(None) else {
                return;
            };
            let params = D2D1_LAYER_PARAMETERS {
                contentBounds: D2D_RECT_F {
                    left: f32::MIN,
                    top: f32::MIN,
                    right: f32::MAX,
                    bottom: f32::MAX,
                },
                geometricMask: ManuallyDrop::new(Some(geometry.0.clone())),
                maskAntialiasMode: D2D1_ANTIALIAS_MODE_PER_PRIMITIVE,
                maskTransform: Matrix3x2::identity(),
                opacity: 1.0,
                opacityBrush: ManuallyDrop::new(None),
                layerOptions: D2D1_LAYER_OPTIONS_NONE,
            };
            self.rt.PushLayer(&params, &layer);
            drop(ManuallyDrop::into_inner(params.geometricMask));
            self.layers.borrow_mut().push(layer);
        }
    }

    /// Removes the clip pushed by [`Target::push_clip_geometry`].
    pub(crate) fn pop_layer(&self) {
        // SAFETY: pops the layer pushed above and releases it afterwards.
        unsafe { self.rt.PopLayer() };
        self.layers.borrow_mut().pop();
    }
}
