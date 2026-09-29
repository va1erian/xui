//! Renders the whole set, headlessly, on a dark and a light background.
//!
//! ```text
//! cargo run -p xui-icons --example gallery [out-dir]
//! ```
//!
//! It writes `village-dark.png` and `village-light.png` into `out-dir`
//! (default `target/snapshots`). No window opens.

use std::path::PathBuf;

use xui_canvas::Surface;
use xui_core::image::Image;
use xui_core::{Color, Rect};
use xui_icons::{Palette, Village, draw};

const COLUMNS: i32 = 6;
const CELL: i32 = 112;
const ICON: i32 = 64;
const MARGIN: i32 = 16;

fn render(background: Color) -> Result<Image, Box<dyn std::error::Error>> {
    let rows = (Village::ALL.len() as i32 + COLUMNS - 1) / COLUMNS;
    let (width, height) = (COLUMNS * CELL + 2 * MARGIN, rows * CELL + 2 * MARGIN);
    let mut surface = Surface::new(width as u32, height as u32);
    surface.fill(background);
    let bounds = Rect::new(0, 0, width, height);
    surface.with_canvas(bounds, |canvas| {
        for (i, &icon) in Village::ALL.iter().enumerate() {
            let (col, row) = (i as i32 % COLUMNS, i as i32 / COLUMNS);
            let left = MARGIN + col * CELL + (CELL - ICON) / 2;
            let top = MARGIN + row * CELL + (CELL - ICON) / 2;
            let rect = Rect::new(left, top, left + ICON, top + ICON);
            draw(canvas, icon, rect, &Palette::GLOBAL_VILLAGE);
        }
    });
    let image = surface.to_image();
    Ok(Image::from_rgba(image.width, image.height, image.pixels)?)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out = PathBuf::from(std::env::args().nth(1).unwrap_or("target/snapshots".into()));
    std::fs::create_dir_all(&out)?;
    render(Color::hex(0x23_1C_5C))?.save_png(out.join("village-dark.png"))?;
    render(Color::hex(0xF3_EE_E0))?.save_png(out.join("village-light.png"))?;
    Ok(())
}
