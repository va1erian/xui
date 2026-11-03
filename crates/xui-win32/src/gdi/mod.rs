#![forbid(unsafe_code)]

//! GDI drawing helpers: RAII handles and a double-buffered paint context.

mod bitmap;
mod brush;
mod cache;
mod font;
mod paint;
mod pen;
mod text_format;

pub use bitmap::Bitmap;
pub use brush::Brush;
pub(crate) use font::system_ui_family;
pub use font::{Font, FontWeight};
pub use paint::{Canvas, Paint};
pub use pen::Pen;
pub use text_format::TextFormat;

pub(crate) use cache::solid_brush as cache_brush;
