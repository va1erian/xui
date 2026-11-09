#![forbid(unsafe_code)]

//! Images: alt text until the app supplies pixels, then a box on the
//! baseline, scaled to fit, hit and selected as one character.

use xui_core::image::Image;

use super::*;
use crate::layout::geom::ImageBox;

/// A `w` x `h` image.
fn bitmap(w: u32, h: u32) -> Arc<Image> {
    Arc::new(Image::from_rgba(w, h, vec![255; (w * h * 4) as usize]).expect("an image"))
}

/// `markdown` laid out `width` wide with `src` supplied as a `w` x `h` image.
fn with_image(markdown: &str, width: f32, src: &str, (w, h): (u32, u32)) -> Layout {
    let mut layout = lay(markdown, width);
    assert!(layout.set_image(src, &bitmap(w, h)));
    layout.ensure(0.0, 1.0e6);
    layout
}

fn images(layout: &Layout, index: usize) -> Vec<ImageBox> {
    layout.block(index).expect("laid out").images.clone()
}

#[test]
fn an_image_without_pixels_is_its_alt_text_and_is_asked_for_once() {
    let mut layout = lay("![a map](map.png) and ![a map](map.png) **x**", 1000.0);
    layout.ensure(0.0, 1.0e6);
    let block = layout.block(0).expect("laid out");
    assert_eq!(texts(block), ["a", "map", "and", "a", "map", "x"]);
    assert!(block.images.is_empty());
    assert_eq!(layout.take_requests(), [Box::from("map.png")]);
    assert!(layout.take_requests().is_empty(), "asked once");
}

#[test]
fn an_image_with_no_alt_text_shows_nothing_until_it_has_pixels() {
    let mut layout = lay("![](dot.png)", 1000.0);
    layout.ensure(0.0, 1.0e6);
    assert!(layout.block(0).expect("laid out").pieces.is_empty());
    assert_eq!(layout.take_requests(), [Box::from("dot.png")]);
    let layout = with_image("![](dot.png)", 1000.0, "dot.png", (10, 10));
    assert_eq!(images(&layout, 0).len(), 1);
}

#[test]
fn an_image_alone_is_its_own_size_and_sets_the_block_height() {
    let layout = with_image("![a map](map.png)", 1000.0, "map.png", (100, 50));
    let block = layout.block(0).expect("laid out");
    assert!(block.pieces.is_empty(), "{:?}", texts(block));
    assert_eq!(block.images[0].rect, FRect::new(0.0, 0.0, 100.0, 50.0));
    assert_eq!(block.images[0].range, 0..5, "its alt text");
    assert_eq!(block.height, 50.0);
}

#[test]
fn a_wide_image_is_scaled_down_to_the_line() {
    let layout = with_image("![wide](w.png)", 400.0, "w.png", (1000, 500));
    assert_eq!(
        images(&layout, 0)[0].rect,
        FRect::new(0.0, 0.0, 400.0, 200.0)
    );
}

#[test]
fn an_image_scales_with_the_dpi() {
    let mut layout = lay("![map](map.png)", 1000.0);
    layout.resize(1000.0, 192);
    layout.set_image("map.png", &bitmap(30, 20));
    layout.ensure(0.0, 1.0e6);
    assert_eq!(images(&layout, 0)[0].rect, FRect::new(0.0, 0.0, 60.0, 40.0));
}

#[test]
fn an_inline_image_stands_on_the_baseline_of_its_line() {
    let layout = with_image("see ![a b](i.png) here", 1000.0, "i.png", (20, 40));
    let block = layout.block(0).expect("laid out");
    assert_eq!(texts(block), ["see", "here"]);
    let image = block.images[0].rect;
    // Its bottom is on the text's baseline (13.5 px into an 18 px line).
    assert_eq!(image, FRect::new(28.0, 0.0, 20.0, 40.0));
    let see = piece(block, "see");
    assert!(close(see.y + see.layout.baseline(), 40.0));
    assert_eq!(piece(block, "here").x, 28.0 + 20.0 + 7.0);
    assert!(close(block.height, 40.0 + 18.0 - 13.5));
}

#[test]
fn an_image_that_does_not_fit_moves_to_the_next_line() {
    let layout = with_image("aaaa ![b](i.png)", 60.0, "i.png", (40, 10));
    let block = layout.block(0).expect("laid out");
    assert_eq!(block.images[0].rect.x, 0.0);
    assert!(block.images[0].rect.y >= 18.0);
}

#[test]
fn a_linked_image_is_clickable_and_not_underlined() {
    let mut layout = with_image("[![logo](l.png)](home.md)", 1000.0, "l.png", (30, 30));
    let block = layout.block(0).expect("laid out");
    assert_eq!(block.links, [(FRect::new(0.0, 0.0, 30.0, 30.0), 0)]);
    assert!(fills(block, Role::Link).is_empty());
    assert_eq!(layout.link_at(15.0, 15.0), Some(0));
}

#[test]
fn a_point_on_an_image_hits_its_nearer_side() {
    let mut layout = with_image("ab ![pic](p.png) cd", 1000.0, "p.png", (40, 40));
    let x = images(&layout, 0)[0].rect.x;
    assert_eq!(layout.pos_at(x + 5.0, 20.0), 3, "the start of its alt text");
    assert_eq!(layout.pos_at(x + 35.0, 20.0), 6, "its end");
}

#[test]
fn a_selected_image_is_in_the_selection_band() {
    let layout = with_image("![pic](p.png)", 1000.0, "p.png", (40, 30));
    let rects = layout.selection_rects(0, &(0..3));
    assert_eq!(rects, [FRect::new(0.0, 0.0, 40.0, 30.0)]);
    assert!(layout.selection_rects(0, &(3..3)).is_empty());
}

#[test]
fn supplying_pixels_lays_the_block_out_again() {
    let mut layout = lay("para\n\n![pic](p.png)", 1000.0);
    layout.ensure(0.0, 1.0e6);
    let before = layout.generation(1);
    assert!(layout.set_image("p.png", &bitmap(10, 90)));
    assert!(layout.block(1).is_none(), "dropped");
    assert!(layout.block(0).is_some(), "other blocks kept");
    assert!(!layout.done(), "the background layout picks it up");
    layout.ensure(0.0, 1.0e6);
    assert_ne!(layout.generation(1), before);
    assert_eq!(layout.block(1).expect("laid out").height, 90.0);
    assert!(!layout.set_image("other.png", &bitmap(1, 1)), "not in it");
}

#[test]
fn an_image_in_a_table_resizes_its_columns() {
    let md = "| a | b |\n|---|---|\n| ![i](i.png) | x |";
    let mut layout = lay(md, 1000.0);
    layout.ensure(0.0, 1.0e6);
    let x_before = piece(layout.block(1).expect("row"), "x").x;
    layout.set_image("i.png", &bitmap(200, 10));
    layout.ensure(0.0, 1.0e6);
    let header = layout.block(0).expect("header laid out again");
    let x_after = piece(layout.block(1).expect("row"), "x").x;
    assert!(x_after > x_before + 150.0, "{x_before} -> {x_after}");
    assert!(close(piece(header, "b").x, x_after));
}

#[test]
fn a_point_beside_text_on_a_tall_line_hits_the_text() {
    let mut layout = with_image("ab ![x y](p.png) cd", 1000.0, "p.png", (40, 60));
    let cd = piece(layout.block(0).expect("laid out"), "cd").x;
    // Level with the image's top, above the text: the text's own positions.
    assert_eq!(layout.pos_at(3.0, 5.0), 0, "above `a`");
    assert_eq!(layout.pos_at(cd + 7.0, 5.0), 8, "between `c` and `d`");
}

#[test]
fn a_selected_line_with_an_image_is_one_band() {
    let layout = with_image("ab ![x y](p.png) cd", 1000.0, "p.png", (40, 60));
    let end = layout.doc().text().len() as u32;
    let rects = layout.selection_rects(0, &(0..end));
    assert_eq!(rects.len(), 1, "{rects:?}");
    assert_eq!(rects[0].y, 0.0);
    assert!(close(rects[0].w, 6.0 * 7.0 + 40.0), "{rects:?}");
}

#[test]
fn alt_text_wraps_as_text_until_the_pixels_come() {
    let mut layout = lay("![one two three four](p.png) **x**", 50.0);
    layout.ensure(0.0, 1.0e6);
    let block = layout.block(0).expect("laid out");
    assert!(block.images.is_empty());
    assert!(block.height >= 3.0 * 18.0, "wrapped: {}", block.height);
}

#[test]
fn an_image_in_a_header_cell_is_drawn() {
    let layout = with_image("| ![h](h.png) |\n|---|\n| x |", 1000.0, "h.png", (30, 20));
    let header = layout.block(0).expect("laid out");
    assert_eq!(header.images.len(), 1);
    assert_eq!(header.images[0].rect.w, 30.0);
}

#[test]
fn an_image_in_a_cell_leaves_the_next_cell_its_own_lines() {
    let md = "| ![i](i.png) | one *two* three four five six seven eight nine ten eleven twelve |\n\
              |---|---|\n";
    let mut layout = with_image(md, 300.0, "i.png", (40, 120));
    let block = layout.block(0).expect("laid out");
    let twelve = piece(block, "twelve");
    let (x, y) = (twelve.x + 3.0, twelve.y + 5.0);
    let five = piece(block, "five").y;
    assert!(twelve.y > five, "the cell wraps");
    let start = layout.doc().text().find("twelve").expect("in the text") as u32;
    assert_eq!(layout.pos_at(x, y), start);
    // Each of the cell's lines is its own band.
    let end = layout.doc().text().len() as u32;
    let rects = layout.selection_rects(0, &(0..end));
    assert!(rects.len() <= 4, "{rects:?}");
}
