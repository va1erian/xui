//! What the random scenes are made of: the generator, the shared description of
//! what a node draws, and the painters.

use std::cell::RefCell;
use std::rc::Rc;

use xui_core::backend::{Canvas, GradientStop, LinearGradient, Rgba};
use xui_core::geometry::{Point, Rect};
use xui_core::image::Image;
use xui_core::{Color, Dip, TextStyle};

/// The window's size in pixels.
pub const W: i32 = 240;
pub const H: i32 = 180;

/// A small deterministic generator (xorshift64*).
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Rng {
        Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
    }

    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    pub fn below(&mut self, bound: usize) -> usize {
        (self.next() % bound as u64) as usize
    }

    pub fn range(&mut self, low: i32, high: i32) -> i32 {
        low + (self.next() % (high - low) as u64) as i32
    }

    pub fn color(&mut self) -> Color {
        Color::rgb(
            self.range(0, 256) as u8,
            self.range(0, 256) as u8,
            self.range(0, 256) as u8,
        )
    }
}

/// What a node's painter draws, shared by both backends so a change is seen
/// by both.
#[derive(Clone)]
pub struct Look {
    pub kind: usize,
    pub color: Color,
    /// How far outside its bounds the ring kind draws.
    pub reach: i32,
    /// Where the caret kind draws its caret, from the left edge.
    pub caret: i32,
}

pub type Looks = Rc<RefCell<Vec<Look>>>;

/// How many kinds of painter there are.
pub const KINDS: usize = 10;

pub fn checker() -> Image {
    let mut pixels = Vec::new();
    for y in 0..8 {
        for x in 0..8 {
            let on = (x / 2 + y / 2) % 2 == 0;
            pixels.extend_from_slice(if on {
                &[220, 40, 40, 255]
            } else {
                &[30, 30, 200, 255]
            });
        }
    }
    Image::from_rgba(8, 8, pixels).expect("image")
}

/// Draws `look` into `bounds`, the node's own rectangle. Everything stays
/// within a few pixels of `bounds`, the contract of a painter.
pub fn draw(canvas: &mut dyn Canvas, bounds: Rect, look: &Look, text: &str, image: &Image) {
    let color = look.color;
    match look.kind {
        0 => canvas.fill_rect(bounds, color),
        1 => {
            canvas.fill_rounded_rect(bounds, 7.0, color);
            canvas.stroke_rounded_rect(bounds, 7.0, Color::rgb(20, 20, 20), 1.0);
        }
        2 => {
            let center = Point::new(
                (bounds.left + bounds.right) / 2,
                (bounds.top + bounds.bottom) / 2,
            );
            canvas.fill_ellipse(
                center,
                (bounds.width() / 2) as f32,
                (bounds.height() / 2) as f32,
                color,
            );
            canvas.draw_line(
                Point::new(bounds.left, bounds.top),
                Point::new(bounds.right, bounds.bottom),
                Color::rgb(0, 0, 0),
                1.0,
            );
        }
        3 => {
            canvas.fill_rect_rgba(bounds, Rgba::with_alpha(color.r, color.g, color.b, 120));
            canvas.stroke_rect(bounds, Color::rgb(0, 80, 0), 1.0);
        }
        4 => canvas.fill_rect_linear(
            bounds,
            &LinearGradient::new(
                Point::new(bounds.left, bounds.top),
                Point::new(bounds.right, bounds.bottom),
                vec![
                    GradientStop::new(0.0, Rgba::from(color)),
                    GradientStop::new(1.0, Rgba::with_alpha(0, 0, 90, 200)),
                ],
            ),
        ),
        5 => {
            canvas.fill_rect(bounds, Color::rgb(245, 245, 235));
            // Text is clipped to the node, as a label that must not overflow is.
            canvas.push_clip(bounds);
            canvas.draw_text(text, bounds, &TextStyle::new(color, Dip(12.0)));
            canvas.pop_clip();
        }
        6 => {
            // A focus ring that sits outside the node's bounds.
            canvas.fill_rect(bounds, color);
            let reach = look.reach.max(1);
            let ring = Rect::new(
                bounds.left - reach,
                bounds.top - reach,
                bounds.right + reach,
                bounds.bottom + reach,
            );
            canvas.stroke_rounded_rect(ring, 5.0, Color::rgb(0, 120, 215), 2.0);
        }
        7 => canvas.draw_image(image, bounds),
        8 => {
            // A caret that moves inside the node.
            canvas.fill_rect(bounds, Color::rgb(250, 250, 250));
            let at = bounds.left + look.caret.min(bounds.width() - 3).max(0);
            canvas.fill_rect(
                Rect::new(at, bounds.top + 2, at + 3, bounds.bottom - 2),
                color,
            );
        }
        _ => canvas.fill_polygon(
            &[
                Point::new(bounds.left, bounds.bottom),
                Point::new(bounds.right, bounds.top + 4),
                Point::new(bounds.left + bounds.width() / 3, bounds.top),
            ],
            color,
        ),
    }
}
