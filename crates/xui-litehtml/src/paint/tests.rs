//! The painter draws where the canvas says its node is.

use xui_canvas::Surface;
use xui_core::Color;
use xui_core::geometry::Rect as PxRect;

use super::Painter;
use crate::geom::{Radius, Rect, Rgba};
use std::sync::Arc;

use crate::list::{Cmd, DisplayList, Image};
use crate::text::TextSystem;

fn red_square() -> DisplayList {
    DisplayList {
        cmds: vec![Cmd::Rect {
            rect: Rect::new(0.0, 0.0, 10.0, 10.0),
            radii: [Radius::default(); 4],
            fill: Rgba::rgb(255, 0, 0),
        }],
        size: (100.0, 100.0),
        fonts: Vec::new(),
        images: Vec::new(),
    }
}

#[test]
fn a_node_away_from_the_origin_paints_at_its_own_corner() {
    let mut surface = Surface::new(200, 120);
    let node = PxRect::new(100, 20, 200, 120);
    let mut painter = Painter::new(TextSystem::for_tests());
    surface.with_canvas(node, |canvas| {
        painter.paint(
            &red_square(),
            canvas,
            Rect::new(0.0, 0.0, 100.0, 100.0),
            0.0,
            Color::rgb(255, 255, 255),
        );
    });
    let image = surface.to_image();
    assert_eq!(image.pixel(105, 25), Some([255, 0, 0, 255]));
    assert_eq!(image.pixel(115, 35), Some([255, 255, 255, 255]));
    // Nothing leaks to the window's origin.
    assert_eq!(image.pixel(5, 5), Some([0, 0, 0, 0]));
}

#[test]
fn an_emptied_image_slot_is_dropped_from_the_cache() {
    let image = |w: u32| {
        Arc::new(Image {
            width: w,
            height: w,
            rgba: vec![255; (w * w * 4) as usize],
        })
    };
    let mut list = red_square();
    list.cmds = vec![Cmd::Image {
        image: 0,
        rect: Rect::new(0.0, 0.0, 4.0, 4.0),
    }];
    list.images = vec![image(4)];
    let mut painter = Painter::new(TextSystem::for_tests());
    let mut surface = Surface::new(20, 20);
    let paint = |painter: &mut Painter, list: &DisplayList, surface: &mut Surface| {
        surface.with_canvas(PxRect::new(0, 0, 20, 20), |canvas| {
            painter.paint(
                list,
                canvas,
                Rect::new(0.0, 0.0, 20.0, 20.0),
                0.0,
                Color::rgb(0, 0, 0),
            );
        });
    };
    paint(&mut painter, &list, &mut surface);
    assert_eq!(painter.images.len(), 1);
    list.images[0] = image(0);
    paint(&mut painter, &list, &mut surface);
    assert!(painter.images.is_empty());
}
