//! A Direct2D render target bound to a window, and the frame lifecycle.

use std::cell::RefCell;

use windows::Win32::Graphics::Direct2D::Common::{
    D2D_SIZE_U, D2D1_ALPHA_MODE_IGNORE, D2D1_COLOR_F, D2D1_PIXEL_FORMAT,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1_FEATURE_LEVEL_DEFAULT, D2D1_HWND_RENDER_TARGET_PROPERTIES,
    D2D1_PRESENT_OPTIONS_RETAIN_CONTENTS, D2D1_RENDER_TARGET_PROPERTIES,
    D2D1_RENDER_TARGET_TYPE_DEFAULT, D2D1_RENDER_TARGET_USAGE_NONE, ID2D1HwndRenderTarget,
    ID2D1Layer, ID2D1SolidColorBrush,
};
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
use windows_numerics::Matrix3x2;
use xui_core::backend::Rgba;

use super::bitmap::BitmapCache;
use super::factory;
use crate::sys::window::Hwnd;

/// Whether a finished frame reached the screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Frame {
    Presented,
    /// The device was lost: drop this target and build a new one.
    Lost,
}

/// The Direct2D target of one window. Drawing is in device pixels: the target
/// runs at 96 DPI, so one Direct2D unit is one pixel.
pub(crate) struct Target {
    pub(super) rt: ID2D1HwndRenderTarget,
    pub(super) brush: ID2D1SolidColorBrush,
    pub(super) bitmaps: RefCell<BitmapCache>,
    pub(super) layers: RefCell<Vec<ID2D1Layer>>,
}

pub(super) fn color(rgba: Rgba) -> D2D1_COLOR_F {
    D2D1_COLOR_F {
        r: f32::from(rgba.r) / 255.0,
        g: f32::from(rgba.g) / 255.0,
        b: f32::from(rgba.b) / 255.0,
        a: f32::from(rgba.a) / 255.0,
    }
}

impl Target {
    /// A target for `hwnd`, sized `width` x `height` device pixels.
    pub(crate) fn new(hwnd: Hwnd, width: i32, height: i32) -> Option<Target> {
        let properties = D2D1_RENDER_TARGET_PROPERTIES {
            r#type: D2D1_RENDER_TARGET_TYPE_DEFAULT,
            pixelFormat: D2D1_PIXEL_FORMAT {
                format: DXGI_FORMAT_B8G8R8A8_UNORM,
                alphaMode: D2D1_ALPHA_MODE_IGNORE,
            },
            dpiX: 96.0,
            dpiY: 96.0,
            usage: D2D1_RENDER_TARGET_USAGE_NONE,
            minLevel: D2D1_FEATURE_LEVEL_DEFAULT,
        };
        let window = D2D1_HWND_RENDER_TARGET_PROPERTIES {
            hwnd: hwnd.raw(),
            pixelSize: D2D_SIZE_U {
                width: width.max(1) as u32,
                height: height.max(1) as u32,
            },
            presentOptions: D2D1_PRESENT_OPTIONS_RETAIN_CONTENTS,
        };
        let rt = factory::with(|f| {
            // SAFETY: both property structs are valid for the call.
            unsafe { f.d2d.CreateHwndRenderTarget(&properties, &window) }.ok()
        })??;
        // SAFETY: a brush owned by `rt`, created once and recoloured per draw.
        let brush = unsafe { rt.CreateSolidColorBrush(&color(Rgba::rgb(0, 0, 0)), None) }.ok()?;
        Some(Target {
            rt,
            brush,
            bitmaps: RefCell::new(BitmapCache::default()),
            layers: RefCell::new(Vec::new()),
        })
    }

    /// Resizes the target to the client area; its contents are lost.
    pub(crate) fn resize(&self, width: i32, height: i32) -> bool {
        let size = D2D_SIZE_U {
            width: width.max(1) as u32,
            height: height.max(1) as u32,
        };
        // SAFETY: `size` is a valid struct.
        unsafe { self.rt.Resize(&size) }.is_ok()
    }

    /// Opens a frame.
    pub(crate) fn begin(&self) {
        // SAFETY: paired with `end`; the caller never nests frames.
        unsafe {
            self.rt.BeginDraw();
            self.rt.SetTransform(&Matrix3x2::identity());
        }
    }

    /// Closes the frame and presents it.
    pub(crate) fn end(&self) -> Frame {
        // SAFETY: closes the frame opened by `begin`.
        match unsafe { self.rt.EndDraw(None, None) } {
            Ok(()) => Frame::Presented,
            Err(_) => Frame::Lost,
        }
    }

    /// Makes `(x, y)` the origin of subsequent drawing, until the next call.
    pub(crate) fn set_origin(&self, x: f32, y: f32) {
        let transform = Matrix3x2 {
            M11: 1.0,
            M12: 0.0,
            M21: 0.0,
            M22: 1.0,
            M31: x,
            M32: y,
        };
        // SAFETY: a plain state change on an open frame.
        unsafe { self.rt.SetTransform(&transform) };
    }

    /// Points the shared brush at `rgba`.
    pub(super) fn solid(&self, rgba: Rgba) -> &ID2D1SolidColorBrush {
        // SAFETY: recolours a brush this target owns.
        unsafe { self.brush.SetColor(&color(rgba)) };
        &self.brush
    }
}
