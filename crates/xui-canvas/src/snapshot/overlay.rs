#![forbid(unsafe_code)]

//! The layout overlay a [`Snapshot`](super::Snapshot) can draw: a one-pixel
//! outline around every rectangle the window's layouts placed.

use xui_core::Rect;

/// The outline colour: magenta, which no theme uses.
const OUTLINE: [u8; 4] = [255, 0, 255, 255];

/// Draws the edges of each of `rects` into top-down RGBA `pixels` of a
/// `width` x `height` image, clipped to the image.
pub(super) fn outline(pixels: &mut [u8], width: u32, height: u32, rects: &[Rect]) {
    let (width, height) = (width as i32, height as i32);
    let mut plot = |x: i32, y: i32| {
        if (0..width).contains(&x) && (0..height).contains(&y) {
            let at = ((y * width + x) * 4) as usize;
            pixels[at..at + 4].copy_from_slice(&OUTLINE);
        }
    };
    for rect in rects.iter().filter(|rect| !rect.is_empty()) {
        let (right, bottom) = (rect.right - 1, rect.bottom - 1);
        for x in rect.left..=right {
            plot(x, rect.top);
            plot(x, bottom);
        }
        for y in rect.top..=bottom {
            plot(rect.left, y);
            plot(right, y);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outlines_only_the_edges_and_clips_to_the_image() {
        let mut pixels = vec![0u8; 4 * 4 * 4];
        outline(
            &mut pixels,
            4,
            4,
            &[Rect::new(1, 1, 3, 3), Rect::new(-5, -5, 1, 1)],
        );
        let at = |x: usize, y: usize| pixels[(y * 4 + x) * 4];
        assert_eq!(at(1, 1), 255);
        assert_eq!(at(2, 2), 255, "a 2x2 rect is all edge");
        assert_eq!(at(3, 3), 0);
        assert_eq!(at(0, 0), 255, "the clipped rect's corner");
    }
}
