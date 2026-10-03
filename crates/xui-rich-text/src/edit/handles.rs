#![forbid(unsafe_code)]

//! Resize handles of a selected image: pure geometry, no backend.

use xui_core::backend::Cursor;
use xui_core::{Dip, Point, Px, Rect};

/// The side of a handle's square.
pub const HANDLE_SIZE: Dip = Dip(8.0);
/// The smallest width or height a resize produces.
pub const MIN_SIZE: Dip = Dip(16.0);

/// One of the eight resize handles around an image.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Handle {
    /// Top-left corner.
    NorthWest,
    /// Top-right corner.
    NorthEast,
    /// Bottom-right corner.
    SouthEast,
    /// Bottom-left corner.
    SouthWest,
    /// Top edge.
    North,
    /// Right edge.
    East,
    /// Bottom edge.
    South,
    /// Left edge.
    West,
}

impl Handle {
    /// All handles, corners first so they win where handles overlap.
    pub const ALL: [Handle; 8] = [
        Handle::NorthWest,
        Handle::NorthEast,
        Handle::SouthEast,
        Handle::SouthWest,
        Handle::North,
        Handle::East,
        Handle::South,
        Handle::West,
    ];

    /// Whether this is a corner handle.
    pub fn is_corner(self) -> bool {
        matches!(
            self,
            Handle::NorthWest | Handle::NorthEast | Handle::SouthEast | Handle::SouthWest
        )
    }

    /// The mouse cursor to show over the handle.
    pub fn cursor(self) -> Cursor {
        match self {
            Handle::NorthWest | Handle::SouthEast => Cursor::SizeNwSe,
            Handle::NorthEast | Handle::SouthWest => Cursor::SizeNeSw,
            Handle::North | Handle::South => Cursor::SizeVertical,
            Handle::East | Handle::West => Cursor::SizeHorizontal,
        }
    }

    /// Which sides move with the pointer: `(x, y)` each -1, 0 or 1 (the sign
    /// the size grows with).
    fn direction(self) -> (i32, i32) {
        match self {
            Handle::NorthWest => (-1, -1),
            Handle::NorthEast => (1, -1),
            Handle::SouthEast => (1, 1),
            Handle::SouthWest => (-1, 1),
            Handle::North => (0, -1),
            Handle::East => (1, 0),
            Handle::South => (0, 1),
            Handle::West => (-1, 0),
        }
    }
}

/// The handles around an image drawn at a given rectangle.
#[derive(Clone, Copy, Debug)]
pub struct Handles {
    image: Rect,
    side: i32,
}

impl Handles {
    /// The handles for an image at `image` (device pixels) at `dpi`.
    pub fn new(image: Rect, dpi: u32) -> Handles {
        Handles {
            image,
            side: HANDLE_SIZE.to_px(dpi).0.max(1),
        }
    }

    /// The square of `handle`, centred on its corner or edge midpoint.
    pub fn rect(&self, handle: Handle) -> Rect {
        let r = self.image;
        let (cx, cy) = ((r.left + r.right) / 2, (r.top + r.bottom) / 2);
        let (dx, dy) = handle.direction();
        let x = match dx {
            -1 => r.left,
            1 => r.right,
            _ => cx,
        };
        let y = match dy {
            -1 => r.top,
            1 => r.bottom,
            _ => cy,
        };
        let half = self.side / 2;
        Rect::new(
            x - half,
            y - half,
            x - half + self.side,
            y - half + self.side,
        )
    }

    /// The handle under `point`, if any.
    pub fn handle_at(&self, point: Point) -> Option<Handle> {
        Handle::ALL
            .into_iter()
            .find(|&handle| self.rect(handle).contains(point))
    }
}

/// The size an image gets when `handle` is dragged by `delta` device pixels
/// from where the drag started, at `start` size.
///
/// Corners keep the aspect ratio unless `keep_aspect` is false (Shift); edges
/// change one dimension. Both dimensions stay at least [`MIN_SIZE`] and the
/// width at most `max_width`.
pub fn resize(
    handle: Handle,
    start: (Dip, Dip),
    delta: (i32, i32),
    dpi: u32,
    keep_aspect: bool,
    max_width: Dip,
) -> (Dip, Dip) {
    let (w, h) = (start.0.0, start.1.0);
    let (sx, sy) = handle.direction();
    let nw = w + sx as f32 * Px(delta.0).to_dip(dpi).0;
    let nh = h + sy as f32 * Px(delta.1).to_dip(dpi).0;
    let (min, max_w) = (MIN_SIZE.0, max_width.0.max(MIN_SIZE.0));
    if handle.is_corner() && keep_aspect && w > 0.0 && h > 0.0 {
        let (rw, rh) = (nw / w, nh / h);
        let scale = if (rw - 1.0).abs() >= (rh - 1.0).abs() {
            rw
        } else {
            rh
        };
        let scale = scale.max(min / w).max(min / h).min(max_w / w);
        return (Dip(w * scale), Dip(h * scale));
    }
    let width = if sx != 0 { nw.clamp(min, max_w) } else { w };
    let height = if sy != 0 { nh.max(min) } else { h };
    (Dip(width), Dip(height))
}

#[cfg(test)]
mod tests {
    use super::*;

    const DPI: u32 = 96;

    fn size(w: f32, h: f32) -> (Dip, Dip) {
        (Dip(w), Dip(h))
    }

    #[test]
    fn handles_sit_on_corners_and_edge_midpoints() {
        let handles = Handles::new(Rect::new(100, 50, 200, 150), DPI);
        assert_eq!(handles.rect(Handle::NorthWest), Rect::new(96, 46, 104, 54));
        assert_eq!(
            handles.rect(Handle::SouthEast),
            Rect::new(196, 146, 204, 154)
        );
        assert_eq!(handles.rect(Handle::North), Rect::new(146, 46, 154, 54));
        assert_eq!(handles.rect(Handle::West), Rect::new(96, 96, 104, 104));
        assert_eq!(
            handles.handle_at(Point::new(198, 148)),
            Some(Handle::SouthEast)
        );
        assert_eq!(handles.handle_at(Point::new(150, 100)), None);
        assert_eq!(handles.handle_at(Point::new(150, 52)), Some(Handle::North));
    }

    #[test]
    fn handles_scale_with_dpi() {
        let handles = Handles::new(Rect::new(0, 0, 200, 200), 192);
        assert_eq!(handles.rect(Handle::SouthEast).width(), 16);
    }

    #[test]
    fn cursors_match_the_direction() {
        assert_eq!(Handle::NorthWest.cursor(), Cursor::SizeNwSe);
        assert_eq!(Handle::SouthWest.cursor(), Cursor::SizeNeSw);
        assert_eq!(Handle::North.cursor(), Cursor::SizeVertical);
        assert_eq!(Handle::East.cursor(), Cursor::SizeHorizontal);
    }

    #[test]
    fn corners_keep_the_aspect_ratio() {
        let out = resize(
            Handle::SouthEast,
            size(100.0, 50.0),
            (50, 0),
            DPI,
            true,
            Dip(1000.0),
        );
        assert_eq!(out, size(150.0, 75.0));
        let out = resize(
            Handle::NorthWest,
            size(100.0, 50.0),
            (0, -50),
            DPI,
            true,
            Dip(1000.0),
        );
        assert_eq!(out, size(200.0, 100.0));
        let out = resize(
            Handle::SouthEast,
            size(100.0, 50.0),
            (50, 10),
            DPI,
            false,
            Dip(1000.0),
        );
        assert_eq!(out, size(150.0, 60.0));
    }

    #[test]
    fn edges_change_one_dimension() {
        let out = resize(
            Handle::East,
            size(100.0, 50.0),
            (30, 99),
            DPI,
            true,
            Dip(1000.0),
        );
        assert_eq!(out, size(130.0, 50.0));
        let out = resize(
            Handle::West,
            size(100.0, 50.0),
            (-20, 0),
            DPI,
            true,
            Dip(1000.0),
        );
        assert_eq!(out, size(120.0, 50.0));
        let out = resize(
            Handle::South,
            size(100.0, 50.0),
            (0, 10),
            DPI,
            true,
            Dip(1000.0),
        );
        assert_eq!(out, size(100.0, 60.0));
    }

    #[test]
    fn size_is_clamped() {
        let out = resize(
            Handle::East,
            size(100.0, 50.0),
            (-500, 0),
            DPI,
            true,
            Dip(300.0),
        );
        assert_eq!(out, size(16.0, 50.0));
        let out = resize(
            Handle::East,
            size(100.0, 50.0),
            (500, 0),
            DPI,
            true,
            Dip(300.0),
        );
        assert_eq!(out, size(300.0, 50.0));
        let out = resize(
            Handle::SouthEast,
            size(100.0, 50.0),
            (-500, -500),
            DPI,
            true,
            Dip(300.0),
        );
        assert_eq!(out, size(32.0, 16.0));
        let out = resize(
            Handle::SouthEast,
            size(100.0, 50.0),
            (900, 0),
            DPI,
            true,
            Dip(300.0),
        );
        assert_eq!(out, size(300.0, 150.0));
    }

    #[test]
    fn the_drag_delta_is_in_device_pixels() {
        let out = resize(
            Handle::East,
            size(100.0, 50.0),
            (96, 0),
            192,
            true,
            Dip(1000.0),
        );
        assert_eq!(out, size(148.0, 50.0));
    }
}
