//! The process-wide Direct2D and DirectWrite factories, created on first use
//! on the UI thread, and the stroke styles built from them.

use std::cell::RefCell;
use std::collections::HashMap;

use windows::Win32::Graphics::Direct2D::{
    D2D1_CAP_STYLE_FLAT, D2D1_CAP_STYLE_ROUND, D2D1_CAP_STYLE_SQUARE, D2D1_DASH_STYLE_DASH,
    D2D1_DASH_STYLE_DOT, D2D1_DASH_STYLE_SOLID, D2D1_FACTORY_TYPE_SINGLE_THREADED,
    D2D1_LINE_JOIN_BEVEL, D2D1_LINE_JOIN_MITER_OR_BEVEL, D2D1_LINE_JOIN_ROUND,
    D2D1_STROKE_STYLE_PROPERTIES, D2D1CreateFactory, ID2D1Factory, ID2D1StrokeStyle,
};
use windows::Win32::Graphics::DirectWrite::{
    DWRITE_FACTORY_TYPE_SHARED, DWriteCreateFactory, IDWriteFactory,
};
use xui_core::backend::{Cap, Dash, Join, Stroke};

pub(crate) struct Factories {
    pub(crate) d2d: ID2D1Factory,
    pub(crate) dwrite: IDWriteFactory,
    strokes: RefCell<HashMap<(Cap, Dash, Join), ID2D1StrokeStyle>>,
}

thread_local! {
    static FACTORIES: Option<Factories> = create();
}

fn create() -> Option<Factories> {
    // SAFETY: creates the two factories; both are used from this thread only
    // (the DirectWrite one is additionally free-threaded, see `text`).
    unsafe {
        Some(Factories {
            d2d: D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None).ok()?,
            dwrite: DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED).ok()?,
            strokes: RefCell::new(HashMap::new()),
        })
    }
}

/// Runs `f` with the thread's factories, or returns `None` when Direct2D or
/// DirectWrite could not be created.
pub(crate) fn with<R>(f: impl FnOnce(&Factories) -> R) -> Option<R> {
    FACTORIES.with(|factories| factories.as_ref().map(f))
}

impl Factories {
    /// The cached style for `stroke`'s cap, dash and join.
    pub(crate) fn stroke_style(&self, stroke: &Stroke) -> Option<ID2D1StrokeStyle> {
        let key = (stroke.cap, stroke.dash, stroke.join);
        if let Some(style) = self.strokes.borrow().get(&key) {
            return Some(style.clone());
        }
        let cap = match stroke.cap {
            Cap::Flat => D2D1_CAP_STYLE_FLAT,
            Cap::Square => D2D1_CAP_STYLE_SQUARE,
            Cap::Round => D2D1_CAP_STYLE_ROUND,
        };
        let properties = D2D1_STROKE_STYLE_PROPERTIES {
            startCap: cap,
            endCap: cap,
            dashCap: cap,
            lineJoin: match stroke.join {
                Join::Miter => D2D1_LINE_JOIN_MITER_OR_BEVEL,
                Join::Bevel => D2D1_LINE_JOIN_BEVEL,
                Join::Round => D2D1_LINE_JOIN_ROUND,
            },
            miterLimit: 10.0,
            dashStyle: match stroke.dash {
                Dash::Solid => D2D1_DASH_STYLE_SOLID,
                Dash::Dashed => D2D1_DASH_STYLE_DASH,
                Dash::Dotted => D2D1_DASH_STYLE_DOT,
            },
            dashOffset: 0.0,
        };
        // SAFETY: `properties` is a valid struct; no custom dash array is used.
        let style = unsafe { self.d2d.CreateStrokeStyle(&properties, None) }.ok()?;
        self.strokes.borrow_mut().insert(key, style.clone());
        Some(style)
    }
}
