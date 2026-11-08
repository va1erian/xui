//! Gradient fills.

use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use windows::Win32::Graphics::Direct2D::Common::D2D1_GRADIENT_STOP;
use windows::Win32::Graphics::Direct2D::{
    D2D1_EXTEND_MODE_CLAMP, D2D1_GAMMA_2_2, D2D1_LINEAR_GRADIENT_BRUSH_PROPERTIES,
    D2D1_RADIAL_GRADIENT_BRUSH_PROPERTIES, ID2D1GradientStopCollection,
};
use windows_numerics::Vector2;
use xui_core::backend::GradientStop;

use super::RectF;
use super::geometry::Geometry;
use super::shapes::rect_f;
use super::target::{Target, color};

fn point(x: f32, y: f32) -> Vector2 {
    Vector2 { X: x, Y: y }
}

/// Most stop collections kept per target.
const MAX_STOPS: usize = 64;

/// Stop collections, keyed by a hash of their stops and used only when the
/// stops compare equal; emptied when full.
#[derive(Default)]
pub(super) struct StopCache(HashMap<u64, (Vec<GradientStop>, ID2D1GradientStopCollection)>);

fn hash_stops(stops: &[GradientStop]) -> u64 {
    let mut hasher = DefaultHasher::new();
    for stop in stops {
        stop.position.to_bits().hash(&mut hasher);
        (stop.color.r, stop.color.g, stop.color.b, stop.color.a).hash(&mut hasher);
    }
    hasher.finish()
}

impl Target {
    /// The collection for `stops`: kept from an earlier fill, or built.
    fn stops(&self, stops: &[GradientStop]) -> Option<ID2D1GradientStopCollection> {
        let hash = hash_stops(stops);
        if let Some((kept, collection)) = self.stops.borrow().0.get(&hash)
            && kept.as_slice() == stops
        {
            return Some(collection.clone());
        }
        let built = self.build_stops(stops)?;
        let mut cache = self.stops.borrow_mut();
        if cache.0.len() >= MAX_STOPS {
            cache.0.clear();
        }
        cache.0.insert(hash, (stops.to_vec(), built.clone()));
        Some(built)
    }

    fn build_stops(&self, stops: &[GradientStop]) -> Option<ID2D1GradientStopCollection> {
        let native: Vec<D2D1_GRADIENT_STOP> = stops
            .iter()
            .map(|stop| D2D1_GRADIENT_STOP {
                position: stop.position.clamp(0.0, 1.0),
                color: color(stop.color),
            })
            .collect();
        // SAFETY: `native` outlives the call, which copies it.
        unsafe {
            self.rt
                .CreateGradientStopCollection(&native, D2D1_GAMMA_2_2, D2D1_EXTEND_MODE_CLAMP)
        }
        .ok()
    }

    pub(crate) fn fill_rect_linear(
        &self,
        rect: RectF,
        start: (f32, f32),
        end: (f32, f32),
        stops: &[GradientStop],
    ) {
        let Some(stops) = self.stops(stops) else {
            return;
        };
        let properties = D2D1_LINEAR_GRADIENT_BRUSH_PROPERTIES {
            startPoint: point(start.0, start.1),
            endPoint: point(end.0, end.1),
        };
        // SAFETY: inside a frame; the brush lives for the call.
        unsafe {
            if let Ok(brush) = self.rt.CreateLinearGradientBrush(&properties, None, &stops) {
                self.rt.FillRectangle(&rect_f(rect), &brush);
            }
        }
    }

    pub(crate) fn fill_rect_radial(
        &self,
        rect: RectF,
        center: (f32, f32),
        radii: (f32, f32),
        stops: &[GradientStop],
    ) {
        let Some(stops) = self.stops(stops) else {
            return;
        };
        let properties = D2D1_RADIAL_GRADIENT_BRUSH_PROPERTIES {
            center: point(center.0, center.1),
            gradientOriginOffset: point(0.0, 0.0),
            radiusX: radii.0,
            radiusY: radii.1,
        };
        // SAFETY: inside a frame; the brush lives for the call.
        unsafe {
            if let Ok(brush) = self.rt.CreateRadialGradientBrush(&properties, None, &stops) {
                self.rt.FillRectangle(&rect_f(rect), &brush);
            }
        }
    }

    pub(crate) fn fill_geometry_linear(
        &self,
        geometry: &Geometry,
        start: (f32, f32),
        end: (f32, f32),
        stops: &[GradientStop],
    ) {
        let Some(stops) = self.stops(stops) else {
            return;
        };
        let properties = D2D1_LINEAR_GRADIENT_BRUSH_PROPERTIES {
            startPoint: point(start.0, start.1),
            endPoint: point(end.0, end.1),
        };
        // SAFETY: inside a frame; the brush lives for the call.
        unsafe {
            if let Ok(brush) = self.rt.CreateLinearGradientBrush(&properties, None, &stops) {
                self.rt.FillGeometry(geometry.raw(), &brush, None);
            }
        }
    }
}
