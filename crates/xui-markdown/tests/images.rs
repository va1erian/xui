//! Images: parsed with their sources, copied as alt text, asked of the app
//! through `on_image` and painted once `set_image` supplies them.

#[path = "view/rig.rs"]
mod rig;

use rig::*;
use xui_core::image::Image;
use xui_markdown::{Document, ImageRef, Inline, OBJECT};

const RED: [u8; 4] = [255, 0, 0, 255];

fn red(w: u32, h: u32) -> Image {
    Image::from_rgba(w, h, RED.repeat((w * h) as usize)).expect("an image")
}

/// How many pixels of `image` are exactly red.
fn reds(image: &Image) -> usize {
    image
        .pixels()
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|p| **p == RED)
        .count()
}

#[test]
fn images_keep_their_source_title_and_alt_text() {
    let doc = Document::parse("![a *b*](x.png \"T\") and ![c](dir/y%20z.png)");
    let refs = [
        ImageRef {
            src: "x.png".into(),
            title: "T".into(),
        },
        ImageRef {
            src: "dir/y%20z.png".into(),
            title: "".into(),
        },
    ];
    assert_eq!(doc.images(), refs);
    let block = &doc.blocks()[0];
    assert_eq!(doc.block_text(block), "a b and c");
    let images: Vec<Option<u32>> = doc.spans(block).iter().map(|s| s.image).collect();
    assert_eq!(images, [Some(0), Some(0), None, Some(1)]);
    assert!(
        doc.spans(block)[1]
            .style
            .contains(Inline::IMAGE.with(Inline::ITALIC))
    );
}

#[test]
fn neighbouring_images_keep_separate_spans() {
    let doc = Document::parse("![a](x.png)![b](x.png)");
    let spans = doc.spans(&doc.blocks()[0]);
    assert_eq!(spans.len(), 2);
    assert_eq!((spans[0].image, spans[1].image), (Some(0), Some(1)));
}

#[test]
fn an_image_with_no_alt_text_holds_an_object_character() {
    let doc = Document::parse("x ![](a.png) y");
    assert_eq!(doc.text(), format!("x {OBJECT} y"));
    let end = doc.text().len() as u32;
    assert_eq!(doc.plain_text(0..end), "x  y", "copied without it");
}

#[test]
fn an_image_in_alt_text_is_part_of_the_outer_image() {
    let doc = Document::parse("![a ![b](in.png) c](out.png)");
    assert_eq!(doc.images().len(), 1);
    assert_eq!(&*doc.image(0).src, "out.png");
    assert_eq!(doc.text(), "a b c");
    assert!(
        doc.spans(&doc.blocks()[0])
            .iter()
            .all(|s| s.image == Some(0))
    );
}

#[test]
fn a_linked_image_carries_both() {
    let doc = Document::parse("[![logo](l.png)](home.md)");
    let span = &doc.spans(&doc.blocks()[0])[0];
    assert_eq!(doc.link(span.link.expect("a link")), "home.md");
    assert_eq!(&*doc.image(span.image.expect("an image")).src, "l.png");
}

/// A frame painted, then a message, so the app takes what the paint queued.
fn frame(stage: &xui_canvas::snapshot::Stage<'_, String>) -> Image {
    let image = stage.ui().capture().expect("capture");
    stage.emit("frame".to_owned());
    image
}

#[test]
fn the_app_is_asked_for_each_source_once_and_its_pixels_are_painted() {
    let md = "Top ![dot](red.png)\n\n![again](red.png) ![](other.png)";
    run_configured(
        md,
        |view| view.on_image(|src| Some(format!("image:{src}"))),
        |stage, rig| {
            let before = frame(stage);
            let asked: Vec<String> = rig
                .log
                .borrow()
                .iter()
                .filter(|m| m.starts_with("image:"))
                .cloned()
                .collect();
            assert_eq!(asked, ["image:red.png", "image:other.png"]);
            assert_eq!(reds(&before), 0, "nothing loaded yet");
            assert!(rig.view.set_image("red.png", red(20, 10)));
            assert!(!rig.view.set_image("nowhere.png", red(1, 1)));
            let after = frame(stage);
            assert_eq!(reds(&after), 2 * 20 * 10, "both uses drawn");
            frame(stage);
            let again = rig
                .log
                .borrow()
                .iter()
                .filter(|m| *m == "image:red.png")
                .count();
            assert_eq!(again, 1, "not asked again");
        },
    );
}

#[test]
fn a_new_document_asks_again_and_drops_old_pixels() {
    run_configured(
        "![a](red.png)",
        |view| view.on_image(|src| Some(format!("image:{src}"))),
        |stage, rig| {
            frame(stage);
            rig.view.set_image("red.png", red(8, 8));
            assert_eq!(reds(&frame(stage)), 64);
            rig.log.borrow_mut().clear();
            rig.view.set_markdown("![b](red.png)");
            let shown = frame(stage);
            assert_eq!(reds(&shown), 0, "the new document has no pixels yet");
            assert_eq!(rig.log.borrow()[0], "image:red.png");
        },
    );
}

#[test]
fn a_selected_image_is_tinted() {
    run_configured(
        "![a](red.png)",
        |view| view.on_image(|_| None),
        |stage, rig| {
            rig.view.set_image("red.png", red(10, 10));
            assert_eq!(reds(&frame(stage)), 100);
            rig.view.select_all();
            assert_eq!(reds(&frame(stage)), 0);
            assert_eq!(rig.view.selected_text(), "a");
        },
    );
}

#[test]
fn a_double_click_on_an_image_selects_all_of_it() {
    run_configured(
        "![two words](red.png)",
        |view| view.on_image(|_| None),
        |stage, rig| {
            rig.view.set_image("red.png", red(60, 40));
            let shot = frame(stage);
            let (w, _) = shot.size();
            let at = (0..w * 60)
                .find(|i| shot.pixel(i % w, i / w) == Some(RED))
                .expect("the image is drawn");
            double_click(stage, (at % w) as i32 + 50, (at / w) as i32 + 20);
            assert_eq!(rig.view.selected_text(), "two words");
        },
    );
}

#[test]
fn an_image_never_supplied_stays_its_alt_text() {
    run_configured(
        "![a](missing.png)",
        |view| view.on_image(|src| Some(format!("image:{src}"))),
        |stage, rig| {
            for _ in 0..3 {
                assert_eq!(reds(&frame(stage)), 0);
            }
            rig.view.select_all();
            assert_eq!(rig.view.selected_text(), "a");
            let asked = rig
                .log
                .borrow()
                .iter()
                .filter(|m| m.starts_with("image:"))
                .count();
            assert_eq!(asked, 1);
        },
    );
}

#[test]
fn a_view_with_no_on_image_still_draws_supplied_pixels() {
    run_configured(
        "![a](red.png)",
        |view| view,
        |stage, rig| {
            assert_eq!(reds(&frame(stage)), 0);
            assert!(rig.view.set_image("red.png", red(4, 4)));
            assert_eq!(reds(&frame(stage)), 16);
        },
    );
}
